#![allow(non_snake_case)]
// Rust wrapper for libsecp256k1, the cryptography library used
// by Bitcoin Core. This crate is maintained by well-known contributors:
// https://github.com/rust-bitcoin/rust-secp256k1/
use secp256k1::{SECP256K1, SecretKey, Keypair, Scalar, XOnlyPublicKey, Parity};

use hex::FromHex;
use rust_decimal::prelude::*;
use sha2::{Sha256, Digest};

use crate::balance::*;

// Nothing-up-my-sleeve point from BIP 341.
// Used as a taproot internal key to effectively disable key-path spends, forcing a script-path spend.
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs
pub fn get_nums() -> XOnlyPublicKey {
    XOnlyPublicKey::from_byte_array(<[u8; 32]>::from_hex("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap()).unwrap()
}

// If the public key derived from a private key has an odd-Y value instead of
// an even-Y value, we need to negate the private key, which will result in
// the required even-Y public key.
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs
pub fn maybe_negate_priv(private_key: SecretKey) -> SecretKey {
    let (_x, parity) = private_key.x_only_public_key(SECP256K1);
    if parity == Parity::Even {
        private_key
    } else {
        private_key.negate()
    }
}

// Given 2 secp256k1 PublicKey objects, construct a 2-of-2 multisig taproot script.
// Return raw bytes of the script (without a length byte prefix)
pub fn create_multisig_script(keys: &[XOnlyPublicKey]) -> Vec<u8> {

}

// Given a single taproot script as a byte array, compute the taptree root hash.
pub fn get_taptree_root(script: &[u8]) -> [u8; 32] {

}

// Given a secp256k1 PublicKey object and a taptree root as bytes, construct the witness program as bytes.
// This is a SegWit version 1 pay-to-taproot witness program:
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_scriptpath_program(internal_key: &XOnlyPublicKey, taptree_root: &[u8; 32]) -> [u8; 34] {
    // Get the 32-byte X coordinate from the internal public key

    // Compute the TapTweak

    // Compute the output public key by tweaking the internal key

    // Compile the version 1 witness program
    // https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki#user-content-Witness_program

}

// Given a secp256k1 PublicKey object and a taptree root as bytes, determine if the
// Y coordinate of the output key (which is a tweaked internal key) is odd.
// This bit is required in the first byte of the control block in the witness.
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-10
pub fn get_p2tr_scriptpath_program_parity_bit(internal_key: &XOnlyPublicKey, taptree_root: &[u8; 32]) -> bool {
    // Get the 32-byte X coordinate from the internal public key

    // Compute the TapTweak

    // Compute the output public key by tweaking the internal key

}

// Given an Outpoint, return a serialized transaction input spending it.
// Use hard-coded defaults for sequence and scriptSig.
pub fn input_from_utxo(outpoint: &Outpoint) -> Vec<u8> {

}

// Given an output script and value (in satoshis), return a serialized transaction output.
pub fn output_from_options(script: &[u8], value: u64) -> Vec<u8> {

}

// Compute the commitment hash for a single input and return bytes to sign.
// This implements the BIP 341 transaction digest algorithm
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#common-signature-message
// We assume only a single input and two outputs, SIGHASH_DEFAULT (which aka SIGHASH_ALL)
// as well as constant default values for sequence and locktime
pub fn get_commitment_hash(outpoint: &[u8; 36], scriptpubkey: &[u8], value: &u64, outputs: &[&[u8]], ext_flag: u8, taptree_root: &Option<[u8; 32]>) -> [u8; 32] {
    // Initialize serialization
    let mut ser: Vec<u8> = Vec::new();
    // Epoch
    // https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-20

    // Control
    //   Hash Type (SIGHASH_DEFAULT aka SIGHASH_ALL)

    // Transaction data
    //   Version, default 2

    //   Locktime, default 0

    //   SHA256 of the serialization of all input outpoints (only one in our case)

    //   SHA256 of the serialization of all input amounts (only one in our case)

    //   SHA256 of all spent outputs' scriptPubKeys including length prefix (only one in our case)

    //   SHA256 of the serialization of all input nSequence, default 0xffffffff (only one in our case)

    //   SHA256 of the serialization of all outputs including length prefix

    // Data about this input
    //   Spend type (0 for key-path spend, 2 for script-path spend)

    //   index of this input in the transaction input vector

    // Data about this output
    //   (nothing needed here for key-path since we are signing SIGHASH_ALL)
    //   For script-path spends, extend the message
    //   https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki#common-signature-message-extension

        // The tapleaf hash as defined in BIP341
        // Since we only have one script in our tree the leaf hash *is* the tree root hash

        // # A constant value 0x00 representing the current version of public keys in the tapscript signature opcode execution.

        // # The opcode position of the last executed OP_CODESEPARATOR (or 0xffffffff if none executed)

    // Return the TapSighash-tagged hash of the aboce serialization

}

// Given the witness program as bytes from a transaction output ScriptPubKey and a wallet State,
// return the secp256k1 SecretKey object we need to sign sith
pub fn get_private_key_for_program(program: &[u8; 34], state: &State) -> SecretKey {

}

// Given a secp256k1 PrivateKey object and TapSighash message as bytes, compute the BIP 340 Schnorr signature.
pub fn sign(private_key: SecretKey, msg: &[u8]) -> [u8; 64] {

}

// Given a secp256k1 PrivateKey object and TapSighash message to sign,
// compute the signature and assemble the serialized p2tr key-path witness
// as defined in BIP 341 (1 stack item only: signature)
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_keypath_witness(private_key: SecretKey, msg: &[u8]) -> Vec<u8> {

}

// Given two secp256k1 PrivateKey objects and a transaction commitment hash to sign,
// compute both signatures and assemble the serialized p2tr script-path witness
// as defined in BIP 341 (4 stack items: signature0, signature1, script, control block)
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_scriptpath_witness(mut private_keys: Vec<SecretKey>, msg: &[u8; 32], parity_bit: bool) -> Vec<u8> {
    let mut witness: Vec<u8> = Vec::new();
    // Push number of witness items

    // Derive public keys first

    // Push signatures into witness in reverse order because they will be un-reversed on the stack

    // Compile script and push bytes to witness

    // Because we only have one script in our taptree, the control block
    // is just the prefix byte plus the internal key, which is our NUMS point.

    witness
}

// Given vectors of inputs, outputs, and witnesses, assemble the complete
// transaction and serialize it for broadcast. Return bytes as hex-encoded string
// suitable to broadcast with Bitcoin Core RPC.
// https://en.bitcoin.it/wiki/Protocol_documentation#tx
pub fn assemble_transaction(inputs: &[&[u8]], outputs: &[&[u8]], witnesses: &[&[u8]]) -> String {

}

// Given arrays of inputs and outputs (no witnesses!) compute the txid.
// Return the 32 byte txid as a *reversed* hex-encoded string.
// https://developer.bitcoin.org/reference/transactions.html#raw-transaction-format
pub fn get_txid(inputs: &[&[u8]], outputs: &[&[u8]]) -> String {

}


// Create a transaction that spends a p2tr key-path utxo to a 2-of-2 multisig
// p2tr script-path program and return both the txid and the final transaction as hex strings
pub fn spend_p2tr_keypath(state: &State) -> (String, String) {
    let FEE = Decimal::new(1, 5); // 0.00001000 BTC
    let AMT = Decimal::new(1, 2); // 0.01000000 BTC
    // Choose an unspent coin worth more than 0.01 BTC

        // Create the input from the utxo

        // Compute destination output script

        // Compute taptree root

        // Compute witness program from taptree root and "NUMS" internal key

        // Get a change output script (one of our wallet's witness programs) and create a change output

        // Get the message to sign

        // Fetch the private key we need to sign with

        // Might need to negate original wallet private key before continuing

        // Tweak the private key according to
        // https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki#tr

        // Sign!

        // Assemble

        // Reserialize without witness data and double-SHA256 to get the txid

    } else {
        panic!("Couldn't find a UTXO to spend");
    }
}


// Create a transaction that spends a 2-of-2 multisig p2tr script-path utxo to a
// an OP_RETURN output that contains YOUR GITHUB HANDLE as a string!
// Return the final transaction as a hex string.
pub fn spend_p2tr_scriptpath(state: &State, txid: &str) -> String {
    let COIN_VALUE = 1000000u64;
    let FEE = 1000u64;
    let AMT = 0u64;
    // Create the input from the utxo
    // Reverse the txid hash from human-readable JSON format to bitcoin wire format

    // Compute destination output script and output

    // Compute change output script and output

    // Get taproot spending data
    //   Compute destination output script

    //   Compute taptree root

    //   Compute witness program from taptree root and "NUMS" internal key

    //   Determine if the output key (which is a tweaked internal key) has an odd Y-coordinate

    // Get the message to sign

    // Sign!

    // Assemble

}
