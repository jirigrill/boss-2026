// Rust wrapper for libsecp256k1, the cryptography library used
// by Bitcoin Core. This crate is maintained by well-known contributors:
// https://github.com/rust-bitcoin/rust-secp256k1/
use secp256k1::{SECP256K1, SecretKey, PublicKey, Scalar, XOnlyPublicKey};

use hmac::{Hmac, Mac};
use num_bigint::BigUint;
use rust_decimal::prelude::*;
use serde_json::Value;
use sha2::{Sha256, Sha512, Digest};
use std::collections::HashMap;
use std::process::Command;

type HmacSha512 = Hmac<Sha512>;

// Protocol-defined Bitcoin primitives
// https://en.bitcoin.it/wiki/Protocol_documentation#Common_structures
// We are only tracking taproot outputs which have a witness program of 34 bytes
pub struct Outpoint {
    pub txid: [u8; 32],
    pub vout: u32
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
        let mut outpoint = Outpoint{ txid: [0u8; 32], vout: 0u32 };
        outpoint.txid.copy_from_slice(&data[0..32]);
        outpoint.vout = u32::from_le_bytes(data[32..36].try_into().unwrap());
        outpoint
    }

    pub fn from_str(txid: &str, vout: u32) -> Self {
        let mut txid_bytes = hex::decode(txid).unwrap();
        txid_bytes.reverse();
        Outpoint{ txid: txid_bytes.try_into().unwrap(), vout}
    }
}

pub struct Utxo {
    pub value: Decimal,
    pub scriptpubkey: [u8; 34]
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
    pub key: [u8; 33]
}

// An intermediate object for deriving BIP32 keys
pub struct Bip32Key {
    pub key: SecretKey,
    pub chaincode: [u8; 32]
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

    // Convert the big integer into bytes

    // Chop off the 32 checksum bits and return
    // BONUS POINTS: Verify the checksum!

}

// Deserialize the extended key bytes and return an ExtendedKey object
pub fn deserialize_key(data: &[u8]) -> ExtendedKey {
    // Initialize ExtendedKey object

    // Copy values from slice into struct arrays

}

// Compute a tagged hash as defined in
// https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki#design
pub fn tagged_hash(data: &[u8], tag: &str) -> [u8; 32] {

}

// Derive the secp256k1 compressed public key from a given private key
// BONUS POINTS: Implement secp256k1 yourself and multiply your key by the generator point!
pub fn get_pub_from_priv(private_key: &SecretKey) -> PublicKey {

}

// Perform a BIP32 parent private key -> child private key operation
// Return a JSON object with "key" and "chaincode" properties as bytes
// https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki#user-content-Private_parent_key_rarr_private_child_key
pub fn derive_priv_child(key: &SecretKey, chaincode: &[u8; 32], mut index: u32, hardened: bool) -> Bip32Key {

}

// Given a master secp256k1 SecretKey object, a chaincode as bytes, and a BIP32 derivation path,
// compute the first 2000 child private keys.
// Return an array of keys encoded as secp256k1 SecretKey objects.
// The derivation path is formatted as an array of (index: int, hardened: bool) tuples.
pub fn get_wallet_privs(key: &SecretKey, chaincode: [u8; 32], path: Vec<(u32, bool)>) -> Vec<SecretKey> {
    // Initialize reusable variables

    // Derive the account key from the path

    // Derive the 2000 "external" keys from the account master

}

// Derive the p2tr witness program for a given public key as a secp256k1 PublicKey object.
// Return a bytes array to be compared with the ScriptPubKey in transaction outputs
// in the JSON response from Bitcoin Core RPC getblock so we can determine
// if the transaction output is "received" by our wallet.
// These are SegWit version 1 pay-to-taproot witness programs.
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_keypath_program(pubkey: &PublicKey) -> [u8; 34] {
    // Get the 32-byte X coordinate from the public key

    // Compute the TapTweak

    // Make sure the internal public key has an even Y coordinate

    // Compute the output public key by tweaking the internal key
    // https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki#tr

    // Compile the version 1 witness program
    // https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki#user-content-Witness_program

}

// Recover the wallet state from the blockchain:
// - Parse xprv and path from descriptor and derive 2000 key pairs and witness programs
// - Request blocks 0-300 from Bitcoin Core via RPC and scan all transactions
// - Return a state object with all the derived keys and total wallet balance
pub fn recover_wallet_state(xprv: &str) -> State {
    // Initialize State


    // Generate all the keypairs and witness programs to search for


    // Scan blocks 0-300

        // Scan every tx in every block


            // Check every tx input for our own utxos, by their outpoint
            // These are coins we have spent.


                // Remove this coin from our wallet state utxo pool
                // so we don't double spend it later and deduct from our balance


            // Check every tx output for our own witness programs.
            // These are coins we have received.


                // Add to our total balance

                // Track this UTXO by its outpoint in case we spend it later

}
