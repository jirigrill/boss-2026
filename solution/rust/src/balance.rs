use hmac::digest::{KeyInit, Update};
// Rust wrapper for libsecp256k1, the cryptography library used
// by Bitcoin Core. This crate is maintained by well-known contributors:
// https://github.com/rust-bitcoin/rust-secp256k1/
use secp256k1::{PublicKey, SECP256K1, Scalar, SecretKey, XOnlyPublicKey};

use hmac::{Hmac, Mac};
use num_bigint::BigUint;
use rust_decimal::prelude::*;
use serde_json::Value;
use sha2::{Digest, Sha256, Sha512};
use std::collections::HashMap;
use std::io::Write;
use std::ops::BitXor;
use std::process::Command;
use std::str::Bytes;
use std::{fmt, vec};

type HmacSha512 = Hmac<Sha512>;

// Protocol-defined Bitcoin primitives
// https://en.bitcoin.it/wiki/Protocol_documentation#Common_structures
// We are only tracking taproot outputs which have a witness program of 34 bytes
pub struct Outpoint {
    pub txid: [u8; 32],
    pub vout: u32,
}
// Serialization helpers since we need to index UTXO by their outpoint
impl Outpoint {
    pub fn serialize(&self) -> [u8; 36] {
        let mut ser = [0u8; 36];
        ser[0..32].copy_from_slice(&self.txid);
        ser[32..36].copy_from_slice(&self.vout.to_le_bytes());
        ser
    }

    pub fn deserialize(data: &[u8; 36]) -> Self {
        let mut outpoint = Outpoint {
            txid: [0u8; 32],
            vout: 0u32,
        };
        outpoint.txid.copy_from_slice(&data[0..32]);
        outpoint.vout = u32::from_le_bytes(data[32..36].try_into().unwrap());
        outpoint
    }

    pub fn from_str(txid: &str, vout: u32) -> Self {
        let mut txid_bytes = hex::decode(txid).unwrap();
        txid_bytes.reverse();
        Outpoint {
            txid: txid_bytes.try_into().unwrap(),
            vout,
        }
    }
}

pub struct Utxo {
    pub value: Decimal,
    pub scriptpubkey: [u8; 34],
}

// Helper to convert BTC (Decimal) to satoshis (u64)
pub fn sats(btc: &Decimal) -> u64 {
    (btc * Decimal::from(100_000_000u64)).to_u64().unwrap()
}

// BIP32 extended key
// https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki#serialization-format
// 4 byte: version bytes (mainnet: 0x0488B21E public, 0x0488ADE4 private; testnet: 0x043587CF public, 0x04358394 private)
// 1 byte: depth: 0x00 for master nodes, 0x01 for level-1 derived keys, ....
// 4 bytes: the fingerprint of the parent's key (0x00000000 if master key)
// 4 bytes: child number. This is ser32(i) for i in xi = xpar/i, with xi the key being serialized. (0x00000000 if master key)
// 32 bytes: the chain code
// 33 bytes: the public key or private key data (serP(K) for public keys, 0x00 || ser256(k) for private keys)
pub struct ExtendedKey {
    pub version: [u8; 4],
    pub depth: [u8; 1],
    pub fingerprint: [u8; 4],
    pub index: [u8; 4],
    pub chaincode: [u8; 32],
    pub key: [u8; 33],
}

impl fmt::Display for ExtendedKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ExtendedKey {{\n")?;
        write!(f, "  version: {}\n", hex::encode(self.version))?;
        write!(f, "  depth: {}\n", self.depth[0])?;
        write!(f, "  fingerprint: {}\n", hex::encode(self.fingerprint))?;
        write!(f, "  index: {}\n", u32::from_be_bytes(self.index))?;
        write!(f, "  chaincode: {}\n", hex::encode(self.chaincode))?;
        write!(f, "  key: {}\n", hex::encode(self.key))?;
        write!(f, "}}")
    }
}

// An intermediate object for deriving BIP32 keys
pub struct Bip32Key {
    pub key: SecretKey,
    pub chaincode: [u8; 32],
}

// Used internally for this wallet program
pub struct State {
    pub utxo: HashMap<[u8; 36], Utxo>,
    pub balance: Decimal,
    pub privs: Vec<SecretKey>,
    pub pubs: Vec<PublicKey>,
    pub programs: Vec<[u8; 34]>,
}

// Assuming Bitcoin Core is running and connected to signet using default datadir,
// execute an RPC and return its value or error message.
// https://github.com/bitcoin/bitcoin/blob/master/doc/bitcoin-conf.md#configuration-file-path
// Examples: bcli("getblockcount")
//           bcli("getblockhash 100")
pub fn bcli(cmd: &str) -> String {
    let res = Command::new("bitcoin-cli")
        .arg("-signet")
        .args(cmd.split_whitespace())
        .output()
        .expect("bitcoin-cli command failed");
    String::from_utf8_lossy(&res.stdout).to_string()
}

// Decode a base58 string into a vector of bytes
pub fn base58_decode(base58_string: &str) -> Vec<u8> {
    let base58_alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    // Convert base58 string into a big integer
    let mut num = BigUint::from(0u32);
    for c in base58_string.chars() {
        num = num * 58u32;
        let digit = base58_alphabet.find(c).unwrap();
        num += digit;
    }

    // Convert the big integer into bytes
    let num_bytes = num.to_bytes_be();

    // Chop off the 32 checksum bits and return
    num_bytes[0..num_bytes.len() - 4].to_vec()
    // BONUS POINTS: Verify the checksum!
}

// Deserialize the extended key bytes and return an ExtendedKey object
pub fn deserialize_key(data: &[u8]) -> ExtendedKey {
    // Initialize ExtendedKey object
    // Copy values from slice into struct arrays
    ExtendedKey {
        version: data[0..4].try_into().unwrap(),
        depth: data[4..5].try_into().unwrap(),
        fingerprint: data[5..9].try_into().unwrap(),
        index: data[9..13].try_into().unwrap(),
        chaincode: data[13..45].try_into().unwrap(),
        key: data[45..78].try_into().unwrap(),
    }
}

// Compute a tagged hash as defined in
// https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki#design
pub fn tagged_hash(data: &[u8], tag: &str) -> [u8; 32] {
    //SHA256(SHA256(tag) || SHA256(tag) || data)
    let tag_hash = Sha256::digest(tag.as_bytes());
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, &tag_hash);
    Digest::update(&mut hasher, &tag_hash);
    Digest::update(&mut hasher, &data);
    hasher.finalize().into()
}

// Derive the secp256k1 compressed public key from a given private key
// BONUS POINTS: Implement secp256k1 yourself and multiply your key by the generator point!
pub fn get_pub_from_priv(private_key: &SecretKey) -> PublicKey {
    PublicKey::from_secret_key_global(private_key)
}

// Perform a BIP32 parent private key -> child private key operation
// Return a JSON object with "key" and "chaincode" properties as bytes
// https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki#user-content-Private_parent_key_rarr_private_child_key
pub fn derive_priv_child(
    key: &SecretKey,
    chaincode: &[u8; 32],
    mut index: u32,
    hardened: bool,
) -> Bip32Key {
    // 1. prepare the index if hardened
    // 2. prepare hmac-sha512 input_data
    // 3. compute hmac-sha512
    // 4. split hmac result into tweak and child_chaincode
    // 5. compute child private key
    // 6. return Bip32Key
    let mut hmac_input_data = Vec::new();

    if hardened {
        // 0x00 || private key bytes (32 bytes) || index (4 bytes, big-endian)
        index |= 0x80000000;
        hmac_input_data.push(0x00);
        hmac_input_data.extend_from_slice(&key.secret_bytes());
        hmac_input_data.extend_from_slice(&index.to_be_bytes());
    } else {
        // compressed public key (33 bytes) || index (4 bytes, big-endian)
        let pub_key = get_pub_from_priv(key);
        hmac_input_data.extend_from_slice(&pub_key.serialize());
        hmac_input_data.extend_from_slice(&index.to_be_bytes());
    }

    let mut hmac = <HmacSha512 as hmac::Mac>::new_from_slice(chaincode).unwrap();
    Mac::update(&mut hmac, &hmac_input_data);
    let hmac_result_bytes = hmac.finalize().into_bytes();

    let tweak: [u8; 32] = hmac_result_bytes[..32].try_into().unwrap();
    let child_chaincode: [u8; 32] = hmac_result_bytes[32..].try_into().unwrap();

    let tweak_scalar = Scalar::from_be_bytes(tweak).unwrap();
    let child_priv_key = key.add_tweak(&tweak_scalar).unwrap();

    Bip32Key {
        chaincode: child_chaincode,
        key: child_priv_key,
    }
}

// Given a master secp256k1 SecretKey object, a chaincode as bytes, and a BIP32 derivation path,
// compute the first 2000 child private keys.
// Return an array of keys encoded as secp256k1 SecretKey objects.
// The derivation path is formatted as an array of (index: int, hardened: bool) tuples.
pub fn get_wallet_privs(
    key: &SecretKey,
    chaincode: [u8; 32],
    path: Vec<(u32, bool)>,
) -> Vec<SecretKey> {
    // Initialize reusable variables
    let mut current_key = *key;
    let mut current_chaincode = chaincode;
    // Derive the account key from the path
    for (index, hardened) in path {
        let derived = derive_priv_child(&current_key, &current_chaincode, index, hardened);
        current_key = derived.key;
        current_chaincode = derived.chaincode;
    }
    // Derive the 2000 "external" keys from the account master
    let mut derived_keys = vec![];
    for i in 0..2000 {
        derived_keys.push(derive_priv_child(&current_key, &current_chaincode, i, false).key);
    }
    derived_keys
}

// Derive the p2tr witness program for a given public key as a secp256k1 PublicKey object.
// Return a bytes array to be compared with the ScriptPubKey in transaction outputs
// in the JSON response from Bitcoin Core RPC getblock so we can determine
// if the transaction output is "received" by our wallet.
// These are SegWit version 1 pay-to-taproot witness programs.
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_keypath_program(pubkey: &PublicKey) -> [u8; 34] {
    // Get the 32-byte X coordinate from the public key
    let (x_coordinate, parity) = pubkey.x_only_public_key();

    // Compute the TapTweak
    let taptweak = tagged_hash(&x_coordinate.serialize(), "TapTweak");

    // Make sure the internal public key has an even Y coordinate
    let mut internal_key = *pubkey;
    if parity.to_i32() == 1 {
        internal_key = internal_key.negate(&SECP256K1);
    }

    // Compute the output public key by tweaking the internal key
    // https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki#tr
    let tr_output_key = internal_key
        .add_exp_tweak(&SECP256K1, &Scalar::from_be_bytes(taptweak).unwrap())
        .unwrap();

    // Compile the version 1 witness program
    // https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki#user-content-Witness_program
    // For P2TR (version 1): 0x51 (OP_1) followed by 0x20 (32 bytes) followed by the 32-byte tweaked X-only pubkey
    let output_x_only = tr_output_key.x_only_public_key().0;
    let mut witness_program = [0u8; 34];
    witness_program[0] = 0x51; // OP_1 (version 1)
    witness_program[1] = 0x20; // 32 bytes length
    witness_program[2..34].copy_from_slice(&output_x_only.serialize());
    witness_program
}

// Recover the wallet state from the blockchain:
// - Parse xprv and path from descriptor and derive 2000 key pairs and witness programs
// - Request blocks 0-300 from Bitcoin Core via RPC and scan all transactions
// - Return a state object with all the derived keys and total wallet balance
pub fn recover_wallet_state(xprv: &str) -> State {
    // Initialize State
    let mut state = State {
        balance: Decimal::new(0, 0),
        privs: vec![],
        programs: vec![],
        pubs: vec![],
        utxo: HashMap::new(),
    };
    let extended_key = deserialize_key(&base58_decode(xprv));
    let master_key_bytes: [u8; 32] = extended_key.key[1..33].try_into().unwrap();
    let master_key = SecretKey::from_byte_array(master_key_bytes).unwrap();
    // Path from email: m/86h/1h/0h/0/*
    let path = vec![(86, true), (1, true), (0, true), (0, false)];

    // Generate all the keypairs and witness programs to search for
    state.privs = get_wallet_privs(&master_key, extended_key.chaincode, path);

    for priv_key in &state.privs {
        let pub_key = get_pub_from_priv(priv_key);
        state.pubs.push(pub_key);
        state.programs.push(get_p2tr_keypath_program(&pub_key));
    }

    // // ========== DEBUG START ==========
    // println!("Generated {} private keys", state.privs.len());
    // println!("Generated {} public keys", state.pubs.len());
    // println!("Generated {} witness programs", state.programs.len());
    // println!("First witness program: {}", hex::encode(&state.programs[0]));
    // println!("Second witness program: {}", hex::encode(&state.programs[1]));
    // println!("Last witness program: {}", hex::encode(&state.programs[1999]));
    // // ========== DEBUG END ==========

    let mut p2tr_outputs_found = 0;

    // Scan blocks 0-300
    for height in 0..=300 {
        let blockhash = bcli(&format!("getblockhash {}", height));
        let block_data = bcli(&format!("getblock {} 2", blockhash.trim()));
        let block: Value = serde_json::from_str(&block_data).unwrap();

        // Scan every tx in every block
        if let Some(transactions) = block["tx"].as_array() {
            for tx in transactions {
                // Check every tx input for our own utxos, by their outpoint
                // These are coins we have spent.
                if let Some(inputs) = tx["vin"].as_array() {
                    for input in inputs {
                        // Skip coinbase transactions (they don't spend UTXOs)
                        if input.get("coinbase").is_some() {
                            continue;
                        }

                        // Extract txid and vout to create outpoint
                        if let (Some(txid_str), Some(vout_num)) =
                            (input["txid"].as_str(), input["vout"].as_u64())
                        {
                            let outpoint = Outpoint::from_str(txid_str, vout_num as u32);
                            let outpoint_ser = outpoint.serialize();

                            // Remove this coin from our wallet state utxo pool
                            // so we don't double spend it later and deduct from our balance
                            if let Some(utxo) = state.utxo.remove(&outpoint_ser) {
                                // // ========== DEBUG START ==========
                                // println!("SPENDING OUR COIN!");
                                // println!("  Block: {}", height);
                                // println!("  Txid: {}", txid_str);
                                // println!("  Vout: {}", vout_num);
                                // println!("  Value spent: {}", utxo.value);
                                // // ========== DEBUG END ==========
                                state.balance -= utxo.value;
                            }
                        }
                    }
                }

                // Check every tx output for our own witness programs.
                // These are coins we have received.
                if let Some(outputs) = tx["vout"].as_array() {
                    for output in outputs {
                        if let Some(scriptpubkey_hex) = output["scriptPubKey"]["hex"].as_str() {
                            let scriptpubkey_bytes = hex::decode(scriptpubkey_hex).unwrap();

                            // Check if this matches any of our witness programs (must be 34 bytes for P2TR)
                            if scriptpubkey_bytes.len() == 34 {
                                let scriptpubkey: [u8; 34] = scriptpubkey_bytes.try_into().unwrap();

                                // Count all P2TR outputs we see
                                if scriptpubkey[0] == 0x51 && scriptpubkey[1] == 0x20 {
                                    p2tr_outputs_found += 1;
                                }

                                if state.programs.contains(&scriptpubkey) {
                                    // // ========== DEBUG START ==========
                                    // println!("MATCH FOUND! RECEIVING COINS!");
                                    // println!("  Block: {}", height);
                                    // println!("  Txid: {}", tx["txid"].as_str().unwrap());
                                    // println!("  Vout: {}", output["n"]);
                                    // println!("  Value: {}", output["value"]);
                                    // println!("  ScriptPubKey: {}", scriptpubkey_hex);
                                    // // ========== DEBUG END ==========

                                    // Add to our total balance
                                    let value =
                                        Decimal::from_f64(output["value"].as_f64().unwrap())
                                            .unwrap();
                                    state.balance += value;

                                    // Track this UTXO by its outpoint in case we spend it later
                                    let txid = tx["txid"].as_str().unwrap();
                                    let vout = output["n"].as_u64().unwrap() as u32;
                                    let outpoint = Outpoint::from_str(txid, vout);

                                    state.utxo.insert(
                                        outpoint.serialize(),
                                        Utxo {
                                            value,
                                            scriptpubkey,
                                        },
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // // ========== DEBUG START ==========
    // println!("\n=== FINAL WALLET STATE ===");
    // println!("Total P2TR outputs found in blocks: {}", p2tr_outputs_found);
    // println!("Total balance: {}", state.balance);
    // println!("Number of UTXOs: {}", state.utxo.len());
    // println!("=========================\n");
    // // ========== DEBUG END ==========

    state
}
