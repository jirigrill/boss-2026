use chacha20::{
    cipher::{KeyIvInit, StreamCipher},
    ChaCha20,
};
use hmac::{Hmac, Mac};
use secp256k1::{
    ecdh::{self, SharedSecret},
    PublicKey, SecretKey, SECP256K1,
};
use serde_with::{hex::Hex, serde_as};
use sha2::{Digest, Sha256};
use std::{
    error::Error,
    fs,
    path::{Path, PathBuf},
};

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

        // ECDH with current ephemeral key
        let ecdh_res = SharedSecret::new(&hop_publickey, &ephemeral_privkey);
        let shared_secret: [u8; 32] = ecdh_res.secret_bytes();
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

    // generate filler
    let mut filler = Vec::new();
    let last_index = json_data.hops.len() - 1;

    for i in 0..last_index {
        let shift_amount = json_data.hops[i].payload.len() + 32;

        //extend filter with zeros
        filler.resize(filler.len() + shift_amount, 0);

        // generate keysream to cover positiion 1300 + filler.len()
        let mut keystream = vec![0u8; 1300 + filler.len()];
        let mut cipher = ChaCha20::new(rho_keys[i].as_ref().into(), &[0u8; 12].into());
        cipher.apply_keystream(&mut keystream);

        // xor entire filler with keystream starting at offset 1300
        for j in 0..filler.len() {
            filler[j] ^= keystream[1300 + j];
        }
    }

    // build the onion
    let mut routing_info = [0u8; 1300];
    let mut current_hmac = [0u8; 32];
    let num_hops = json_data.hops.len();

    for i in (0..num_hops).rev() {
        let payload = &json_data.hops[i].payload;
        let shift_amount = payload.len() + 32;

        // 1. shift right - make space in the front
        routing_info.copy_within(0..1300 - shift_amount, shift_amount);

        // 2, insert payload in position 0
        routing_info[0..payload.len()].copy_from_slice(payload);

        // 3. insert current hmac after payload
        routing_info[payload.len()..payload.len() + 32].copy_from_slice(&current_hmac);

        // 4. encrypt with chacha20
        let mut cipher = ChaCha20::new(rho_keys[i].as_ref().into(), &[0u8; 12].into());
        cipher.apply_keystream(&mut routing_info);

        // 5. instert filler - only on first iteration
        if i == num_hops - 1 {
            let filler_start = 1300 - filler.len();
            routing_info[filler_start..].copy_from_slice(&filler);
        }

        // 6. compute hmac
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

    println!("onion: {}", hex::encode(&onion));

    Ok(())
}
