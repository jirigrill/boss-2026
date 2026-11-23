use chacha20::{
    cipher::{KeyIvInit, StreamCipher},
    ChaCha20,
};
use hmac::{Hmac, Mac};
use secp256k1::{PublicKey, Scalar, SecretKey, SECP256K1};
use serde_with::{hex::Hex, serde_as};
use sha2::{Digest, Sha256};
use std::{error::Error, fs, path::PathBuf};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug)]
struct Args {
    output_dir: PathBuf,
    input_json: PathBuf,
}

// values are converted from hex in String to byte arrays with serde_with crate
#[serde_as]
#[derive(Debug, serde::Deserialize)]
struct Input {
    #[serde_as(as = "Hex")]
    session_key: [u8; 32],
    #[serde_as(as = "Hex")]
    associated_data: [u8; 32],
    hops: Vec<Hop>,
}

#[serde_as]
#[derive(Debug, serde::Deserialize)]
struct Hop {
    #[serde_as(as = "Hex")]
    pubkey: [u8; 33],
    #[serde_as(as = "Hex")]
    payload: Vec<u8>,
}

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let output_dir: PathBuf = args.next().ok_or("Expected a directory")?.into();
    let input_json: PathBuf = args.next().ok_or("Expected a path")?.into();

    Ok(Args {
        output_dir,
        input_json,
    })
}

fn derive_key(shared_secret: &[u8; 32], key_type: &[u8]) -> [u8; 32] {
    let mut mac = HmacSha256::new_from_slice(key_type).unwrap();
    mac.update(shared_secret);
    mac.finalize().into_bytes().into()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // parse args and load input data into structs
    let args = parse_args()?;
    let file = fs::File::open(args.input_json).expect("Can't open a file");
    let json_data: Input = serde_json::from_reader(file).expect("Can't read a JSON");

    // generate ephemeral public key
    let secret_key = SecretKey::from_byte_array(json_data.session_key)?;
    let public_key = PublicKey::from_secret_key(SECP256K1, &secret_key).serialize();

    // generate shared secrets for all hops
    let mut shared_secrets = Vec::new();
    let mut ephemeral_privkey = secret_key;
    let mut ephemeral_pubkey = public_key;

    for hop in json_data.hops.iter() {
        let hop_publickey = PublicKey::from_slice(&hop.pubkey)?;

        // ECDH with current ephemeral key - compute shared point and hash it
        let scalar = Scalar::from(ephemeral_privkey);
        let shared_point = hop_publickey.mul_tweak(SECP256K1, &scalar)?;
        let shared_bytes = shared_point.serialize(); // Compressed, 33 bytes
        let shared_secret: [u8; 32] = Sha256::digest(&shared_bytes).into();
        shared_secrets.push(shared_secret);

        // blinding factor: SHA256(ephemeral_pubkey || shared_secret)
        let mut blinding_input = Vec::new();
        blinding_input.extend_from_slice(&ephemeral_pubkey);
        blinding_input.extend_from_slice(&shared_secret);
        let blinding_factor: [u8; 32] = Sha256::digest(blinding_input).into();

        //blind private key: ephemeral_privkey = ephemeral_privkey * blinding_factor
        let blinding_scalar = SecretKey::from_byte_array(blinding_factor)?;
        ephemeral_privkey = ephemeral_privkey.mul_tweak(&blinding_scalar.into())?;

        // derive new public key
        ephemeral_pubkey = PublicKey::from_secret_key(SECP256K1, &ephemeral_privkey).serialize();
    }

    // derive encryption keys from shared secrets
    let mut rho_keys = Vec::new();
    let mut mu_keys = Vec::new();
    for ss in shared_secrets.iter() {
        rho_keys.push(derive_key(ss, b"rho"));
        mu_keys.push(derive_key(ss, b"mu"));
    }

    // generate filler using cumulative sizes
    let num_hops = json_data.hops.len();

    // 1. calculate hop sizes (payload + HMAC) and cumulative sizes
    let hop_sizes: Vec<usize> = json_data
        .hops
        .iter()
        .map(|h| h.payload.len() + 32)
        .collect();
    let mut cumulative_sizes: Vec<usize> = Vec::with_capacity(num_hops);
    for i in 0..num_hops {
        cumulative_sizes.push(hop_sizes.iter().take(i).sum());
    }

    let mut filler_accumulators: Vec<Vec<u8>> = Vec::with_capacity(num_hops);

    for i in 0..(num_hops - 1) {
        // 2. generate keystream of 2600 bytes (extends beyond packet for filler computation)
        let mut keystream = vec![0u8; 2600];
        let mut cipher = ChaCha20::new(rho_keys[i].as_ref().into(), &[0u8; 12].into());
        cipher.apply_keystream(&mut keystream);

        // 3. calculate filler boundaries using cumulative sizes
        let filler_start = 1300 - cumulative_sizes[i];
        let filler_end = 1300 + json_data.hops[i].payload.len();
        let filler_length = filler_end + 32 - filler_start;

        // 4. xor zeros with keystream at filler position
        let mut this_filler = vec![0u8; filler_length];
        for j in filler_start..(filler_end + 32) {
            let idx = j - filler_start;
            this_filler[idx] ^= keystream[j];
        }

        // 5. accumulate with previous filler (XOR chain)
        if i == 0 {
            filler_accumulators.push(this_filler);
        } else {
            let prev_filler = &filler_accumulators[i - 1];
            let mut filler_acc = this_filler;
            for j in 0..cumulative_sizes[i] {
                filler_acc[j] ^= prev_filler[j];
            }
            filler_accumulators.push(filler_acc);
        }
    }

    let filler = if num_hops > 1 {
        filler_accumulators.last().cloned().unwrap_or_default()
    } else {
        Vec::new()
    };

    // build the onion
    // 1. initialize routing_info with pad_key encrypted zeros (pseudo-random padding)
    let pad_key = derive_key(&json_data.session_key, b"pad");
    let mut routing_info = [0u8; 1300];
    let mut pad_cipher = ChaCha20::new(pad_key.as_ref().into(), &[0u8; 12].into());
    pad_cipher.apply_keystream(&mut routing_info);

    let mut current_hmac = [0u8; 32];

    // pre-generate rho streams (2600 bytes each) for onion construction
    let mut rho_streams: Vec<Vec<u8>> = Vec::with_capacity(num_hops);
    for i in 0..num_hops {
        let mut stream = vec![0u8; 2600];
        let mut cipher = ChaCha20::new(rho_keys[i].as_ref().into(), &[0u8; 12].into());
        cipher.apply_keystream(&mut stream);
        rho_streams.push(stream);
    }

    for i in (0..num_hops).rev() {
        let payload = &json_data.hops[i].payload;
        let shift_amount = payload.len() + 32;

        // 2. shift right - make space in the front
        routing_info.copy_within(0..1300 - shift_amount, shift_amount);

        // 3. insert payload in position 0
        routing_info[0..payload.len()].copy_from_slice(payload);

        // 4. insert current hmac after payload
        routing_info[payload.len()..payload.len() + 32].copy_from_slice(&current_hmac);

        // 5. encrypt with chacha20 (XOR with rho stream)
        for j in 0..1300 {
            routing_info[j] ^= rho_streams[i][j];
        }

        // 6. insert filler - only on last hop (first iteration in reverse)
        if i == num_hops - 1 {
            let filler_start = 1300 - filler.len();
            routing_info[filler_start..].copy_from_slice(&filler);
        }

        // 7. compute hmac
        let mut mac = HmacSha256::new_from_slice(&mu_keys[i]).unwrap();
        mac.update(&routing_info);
        mac.update(&json_data.associated_data);
        current_hmac = mac.finalize().into_bytes().into();
    }

    let mut onion = Vec::with_capacity(1366);
    onion.push(0x00); // Version byte
    onion.extend_from_slice(&public_key); // Original ephemeral pubkey (33 bytes)
    onion.extend_from_slice(&routing_info); // Encrypted routing info (1300 bytes)
    onion.extend_from_slice(&current_hmac); // Final HMAC (32 bytes)

    // Write output to file
    fs::create_dir_all(args.output_dir.as_path())?;
    let output_path = args.output_dir.join("output.txt");
    fs::write(&output_path, hex::encode(&onion))?;

    Ok(())
}
