#![allow(non_snake_case)]
#![allow(warnings)]
use std::{io::Read, str::Bytes};

// Rust wrapper for libsecp256k1, the cryptography library used
// by Bitcoin Core. This crate is maintained by well-known contributors:
// https://github.com/rust-bitcoin/rust-secp256k1/
use secp256k1::{Keypair, Parity, SECP256K1, Scalar, SecretKey, XOnlyPublicKey};

use hex::FromHex;
use rust_decimal::prelude::*;
use sha2::{Digest, Sha256};

use crate::balance::*;

// Nothing-up-my-sleeve point from BIP 341.
// Used as a taproot internal key to effectively disable key-path spends, forcing a script-path spend.
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs
pub fn get_nums() -> XOnlyPublicKey {
    XOnlyPublicKey::from_byte_array(
        <[u8; 32]>::from_hex("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0")
            .unwrap(),
    )
    .unwrap()
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
    //0x20 <pubkey1> OP_CHECKSIGVERIFY 0x20 <pubkey2> OP_CHECKSIG
    let mut multisig = Vec::new();
    multisig.push(0x20); //PUSH 32 bytes
    multisig.extend_from_slice(&keys[0].serialize());
    multisig.push(0xad); //OP_CHECKSIGVERIFY
    multisig.push(0x20); //PUSH 32 bytes
    multisig.extend_from_slice(&keys[1].serialize());
    multisig.push(0xac);
    multisig
}

fn compact_size(len: usize) -> u8 {
    assert!(len < 253);
    len as u8
}

// Given a single taproot script as a byte array, compute the taptree root hash.
pub fn get_taptree_root(script: &[u8]) -> [u8; 32] {
    // leaf_version || compact_size(script_length) || script
    let mut leaf_data = vec![];
    leaf_data.push(0xc0); // leaf version
    leaf_data.push(compact_size(script.len()));
    leaf_data.extend_from_slice(script);
    let taptree_root = tagged_hash(&leaf_data, "TapLeaf");
    taptree_root
}

// Given a secp256k1 PublicKey object and a taptree root as bytes, construct the witness program as bytes.
// This is a SegWit version 1 pay-to-taproot witness program:
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_scriptpath_program(
    internal_key: &XOnlyPublicKey,
    taptree_root: &[u8; 32],
) -> [u8; 34] {
    // Get the 32-byte X coordinate from the internal public key
    let mut tweak_data = [0u8; 64];
    tweak_data[..32].copy_from_slice(&internal_key.serialize());
    tweak_data[32..].copy_from_slice(taptree_root);

    // Compute the TapTweak
    let tap_tweak = tagged_hash(&tweak_data, "TapTweak");

    // Compute the output public key by tweaking the internal key
    let (output_key, _parity) = internal_key
        .add_tweak(SECP256K1, &Scalar::from_be_bytes(tap_tweak).unwrap())
        .unwrap();

    // Compile the version 1 witness program
    // https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki#user-content-Witness_program
    let mut output_script = [0u8; 34];
    output_script[0] = 0x51; // OP_1
    output_script[1] = 0x20; // PUSH 32 bytes
    output_script[2..].copy_from_slice(&output_key.serialize());
    output_script
}

// Given a secp256k1 PublicKey object and a taptree root as bytes, determine if the
// Y coordinate of the output key (which is a tweaked internal key) is odd.
// This bit is required in the first byte of the control block in the witness.
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-10
pub fn get_p2tr_scriptpath_program_parity_bit(
    internal_key: &XOnlyPublicKey,
    taptree_root: &[u8; 32],
) -> bool {
    // Get the 32-byte X coordinate from the internal public key
    let x_coordinate = internal_key.serialize();

    let mut tweak_data = [0u8; 64];
    tweak_data[..32].copy_from_slice(&x_coordinate);
    tweak_data[32..].copy_from_slice(taptree_root);
    // Compute the TapTweak
    let tap_tweak = tagged_hash(&tweak_data, "TapTweak");

    // Compute the output public key by tweaking the internal key
    let (output_key, parity) = internal_key
        .add_tweak(SECP256K1, &Scalar::from_be_bytes(tap_tweak).unwrap())
        .unwrap();

    parity == Parity::Odd
}

// Given an Outpoint, return a serialized transaction input spending it.
// Use hard-coded defaults for sequence and scriptSig.
pub fn input_from_utxo(outpoint: &Outpoint) -> Vec<u8> {
    let mut input = Vec::new();
    input.extend_from_slice(&outpoint.txid);
    input.extend_from_slice(&outpoint.vout.to_le_bytes());
    input.push(0x00); // scriptSig
    input.extend_from_slice(&0xffffffff_u32.to_le_bytes()); //sequence
    input
}

// Given an output script and value (in satoshis), return a serialized transaction output.
pub fn output_from_options(script: &[u8], value: u64) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&value.to_le_bytes());
    output.push(compact_size(script.len()));
    output.extend_from_slice(script);
    output
}

// Compute the commitment hash for a single input and return bytes to sign.
// This implements the BIP 341 transaction digest algorithm
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#common-signature-message
// We assume only a single input and two outputs, SIGHASH_DEFAULT (which aka SIGHASH_ALL)
// as well as constant default values for sequence and locktime
pub fn get_commitment_hash(
    outpoint: &[u8; 36],
    scriptpubkey: &[u8],
    value: &u64,
    outputs: &[&[u8]],
    ext_flag: u8,
    taptree_root: &Option<[u8; 32]>,
) -> [u8; 32] {
    // Initialize serialization
    let mut ser: Vec<u8> = Vec::new();
    // Epoch
    // https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-20
    ser.push(0x00);

    // Control
    //   Hash Type (SIGHASH_DEFAULT aka SIGHASH_ALL)
    ser.push(0x00);

    // Transaction data
    //   Version, default 2
    ser.extend_from_slice(&2u32.to_le_bytes());

    //   Locktime, default 0
    ser.extend_from_slice(&0u32.to_le_bytes());

    //   SHA256 of the serialization of all input outpoints (only one in our case)
    ser.extend_from_slice(&Sha256::digest(outpoint));

    //   SHA256 of the serialization of all input amounts (only one in our case)
    ser.extend_from_slice(&Sha256::digest(value.to_le_bytes()));

    //   SHA256 of all spent outputs' scriptPubKeys including length prefix (only one in our case)
    let serialized_spk = [&[compact_size(scriptpubkey.len())], scriptpubkey].concat();
    ser.extend_from_slice(&Sha256::digest(&serialized_spk));

    //   SHA256 of the serialization of all input nSequence, default 0xffffffff (only one in our case)
    ser.extend_from_slice(&Sha256::digest(&0xffffffffu32.to_le_bytes()));

    //   SHA256 of the serialization of all outputs including length prefix
    ser.extend_from_slice(&Sha256::digest(&outputs.concat()));

    // Data about this input
    //   Spend type (0 for key-path spend, 2 for script-path spend)
    ser.push(ext_flag * 2);

    //   index of this input in the transaction input vector
    ser.extend_from_slice(&0u32.to_le_bytes());

    // Data about this output
    //   (nothing needed here for key-path since we are signing SIGHASH_ALL)
    //   For script-path spends, extend the message
    //   https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki#common-signature-message-extension
    if taptree_root.is_some() {
        // The tapleaf hash as defined in BIP341
        // Since we only have one script in our tree the leaf hash *is* the tree root hash
        ser.extend_from_slice(&taptree_root.unwrap());
        // # A constant value 0x00 representing the current version of public keys in the tapscript signature opcode execution.
        ser.push(0x00);
        // # The opcode position of the last executed OP_CODESEPARATOR (or 0xffffffff if none executed)
        ser.extend_from_slice(&0xffffffffu32.to_le_bytes());
    }
    // Return the TapSighash-tagged hash of the aboce serialization
    tagged_hash(&ser, "TapSighash")
}

// Given the witness program as bytes from a transaction output ScriptPubKey and a wallet State,
// return the secp256k1 SecretKey object we need to sign sith
pub fn get_private_key_for_program(program: &[u8; 34], state: &State) -> SecretKey {
    let index = state
        .programs
        .iter()
        .position(|prg| prg == program)
        .expect("No matching program found");
    state.privs[index]
}

// Given a secp256k1 PrivateKey object and TapSighash message as bytes, compute the BIP 340 Schnorr signature.
pub fn sign(private_key: SecretKey, msg: &[u8]) -> [u8; 64] {
    let keypair = private_key.keypair(SECP256K1);
    let schnorr_sig = SECP256K1.sign_schnorr_no_aux_rand(&msg, &keypair);
    schnorr_sig.to_byte_array()
}

// Given a secp256k1 PrivateKey object and TapSighash message to sign,
// compute the signature and assemble the serialized p2tr key-path witness
// as defined in BIP 341 (1 stack item only: signature)
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_keypath_witness(private_key: SecretKey, msg: &[u8]) -> Vec<u8> {
    let mut witness = Vec::new();
    let signed = sign(private_key, msg);
    witness.push(0x01); // number of stack items - only 1 for key-path spend
    witness.push(0x40); // length of signature (64 bytes)
    witness.extend_from_slice(&signed);
    witness
}

// Given two secp256k1 PrivateKey objects and a transaction commitment hash to sign,
// compute both signatures and assemble the serialized p2tr script-path witness
// as defined in BIP 341 (4 stack items: signature0, signature1, script, control block)
// https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
pub fn get_p2tr_scriptpath_witness(
    mut private_keys: Vec<SecretKey>,
    msg: &[u8; 32],
    parity_bit: bool,
) -> Vec<u8> {
    let mut witness: Vec<u8> = Vec::new();
    // Push number of witness items
    witness.push(0x04); //4 items

    // Derive public keys first
    let mut public_keys = Vec::new();
    for privkey in &private_keys {
        public_keys.push(privkey.x_only_public_key(SECP256K1).0);
    }

    // Push signatures into witness in reverse order because they will be un-reversed on the stack
    let mut signatures = Vec::new();
    for privkey in private_keys.iter().rev() {
        signatures.push(sign(*privkey, msg));
    }

    // Compile script and push bytes to witness
    for signature in signatures {
        witness.push(0x40); // 64 bytes
        witness.extend_from_slice(&signature);
    }

    let script = create_multisig_script(&public_keys);
    witness.push(compact_size(script.len()));
    witness.extend_from_slice(&script);

    // Because we only have one script in our taptree, the control block
    // is just the prefix byte plus the internal key, which is our NUMS point.
    let mut control_block = Vec::new();
    // First byte: leaf version (0xc0) OR'd with parity bit
    control_block.push(0xc0 | if parity_bit { 0x01 } else { 0x00 });
    // Next 32 bytes: the internal key (NUMS point)
    control_block.extend_from_slice(&get_nums().serialize());

    witness.push(compact_size(control_block.len()));
    witness.extend_from_slice(&control_block);

    // witness
    witness
}

// Given vectors of inputs, outputs, and witnesses, assemble the complete
// transaction and serialize it for broadcast. Return bytes as hex-encoded string
// suitable to broadcast with Bitcoin Core RPC.
// https://en.bitcoin.it/wiki/Protocol_documentation#tx
pub fn assemble_transaction(inputs: &[&[u8]], outputs: &[&[u8]], witnesses: &[&[u8]]) -> String {
    let mut complete_tx = Vec::new();
    complete_tx.extend_from_slice(&2u32.to_le_bytes()); // version 2
    complete_tx.push(0x00); // segwit marker
    complete_tx.push(0x01); // segwit flag
    complete_tx.push(compact_size(inputs.len())); // input count
    complete_tx.extend_from_slice(&inputs.concat()); //inputs
    complete_tx.push(compact_size(outputs.len())); // output count
    complete_tx.extend_from_slice(&outputs.concat()); //outputs
    complete_tx.extend_from_slice(&witnesses.concat()); //witnesses
    complete_tx.extend_from_slice(&0u32.to_le_bytes()); //locktime

    hex::encode(complete_tx)
}

// Given arrays of inputs and outputs (no witnesses!) compute the txid.
// Return the 32 byte txid as a *reversed* hex-encoded string.
// https://developer.bitcoin.org/reference/transactions.html#raw-transaction-format
pub fn get_txid(inputs: &[&[u8]], outputs: &[&[u8]]) -> String {
    let mut txid = Vec::new();
    txid.extend_from_slice(&2u32.to_le_bytes()); // version 2
    txid.push(compact_size(inputs.len())); // input count
    txid.extend_from_slice(&inputs.concat()); //inputs
    txid.push(compact_size(outputs.len())); // output count
    txid.extend_from_slice(&outputs.concat()); //outputs
    txid.extend_from_slice(&0u32.to_le_bytes()); //locktime
    txid = Sha256::digest(Sha256::digest(txid)).to_vec();
    txid.reverse();

    hex::encode(txid)
}

// Create a transaction that spends a p2tr key-path utxo to a 2-of-2 multisig
// p2tr script-path program and return both the txid and the final transaction as hex strings
pub fn spend_p2tr_keypath(state: &State) -> (String, String) {
    let FEE = Decimal::new(1, 5); // 0.00001000 BTC
    let AMT = Decimal::new(1, 2); // 0.01000000 BTC
    // Choose an unspent coin worth more than 0.01 BTC
    let (outpoint_bytes, utxo) = state
        .utxo
        .iter()
        .find(|(_, utxo)| utxo.value >= AMT)
        .expect("Couldn't find suitable UTXO");
    let outpoint = Outpoint::deserialize(outpoint_bytes);

    // Create the input from the utxo
    let spend_tx_input = input_from_utxo(&outpoint);

    // Compute destination output script
    let pubkey1 = state.pubs[0].x_only_public_key().0;
    let pubkey2 = state.pubs[1].x_only_public_key().0;

    // println!("pubkey1: {}", hex::encode(&pubkey1.serialize()));
    // println!("pubkey2: {}", hex::encode(&pubkey2.serialize()));

    let multisig = create_multisig_script(&[pubkey1, pubkey2]);

    // println!("multisig: {}", hex::encode(&multisig));

    // Compute taptree root
    let taproot_root = get_taptree_root(&multisig);

    // println!("taproot root: {}", hex::encode(&taproot_root));

    // Compute witness program from taptree root and "NUMS" internal key
    let witness_program = get_p2tr_scriptpath_program(&get_nums(), &taproot_root);

    // println!("witness program: {}", hex::encode(&witness_program));

    let input_satoshi = sats(&utxo.value);

    // Get a change output script (one of our wallet's witness programs) and create a change output
    let change_sathoshi = sats(&(utxo.value - AMT - FEE));
    let change_output = output_from_options(&state.programs[0], change_sathoshi);

    // println!("change output: {}", hex::encode(&change_output));

    let output_satoshi = sats(&AMT);
    let output = output_from_options(&witness_program, output_satoshi);

    // println!("output: {}", hex::encode(&output));

    // Get the message to sign
    let message = get_commitment_hash(
        outpoint_bytes,
        &utxo.scriptpubkey,
        &input_satoshi,
        &[&output, &change_output],
        0,
        &None,
    );

    // println!("message: {}", hex::encode(&message));

    // Fetch the private key we need to sign with
    let mut privkey = get_private_key_for_program(&utxo.scriptpubkey, &state);

    // Might need to negate original wallet private key before continuing
    privkey = maybe_negate_priv(privkey);

    // println!("private key: {}", hex::encode(&privkey.secret_bytes()));

    // Tweak the private key according to
    // https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki#tr
    let internal_pubkey = privkey.x_only_public_key(SECP256K1).0;
    let tweak = tagged_hash(&internal_pubkey.serialize(), "TapTweak");
    let tweaked_privkey = privkey
        .add_tweak(&Scalar::from_be_bytes(tweak).unwrap())
        .unwrap();

    //  println!(
    //     "tweaked private key: {}",
    //     hex::encode(&tweaked_privkey.secret_bytes())
    // );

    // Sign!
    let witness = get_p2tr_keypath_witness(tweaked_privkey, &message);

    // println!("witness: {}", hex::encode(&witness));

    // Assemble
    let assembled_tx =
        assemble_transaction(&[&spend_tx_input], &[&output, &change_output], &[&witness]);

    // println!("assembled transaction: {}", assembled_tx);

    // Reserialize without witness data and double-SHA256 to get the txid
    let tx_id = get_txid(&[&spend_tx_input], &[&output, &change_output]);

    // println!("tx id: {}", tx_id);

    (tx_id, assembled_tx)
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
    let mut txid_bytes = <[u8; 32]>::from_hex(txid).unwrap();
    txid_bytes.reverse();

    let outpoint = Outpoint {
        txid: txid_bytes,
        vout: 0,
    };

    let spend_tx_input = input_from_utxo(&outpoint);

    // Compute destination output script and output
    let github_handle = "jirigrill";
    let mut op_return_script = Vec::new();
    op_return_script.push(0x6a); //OP_RETURN
    op_return_script.push(compact_size(github_handle.len()));
    op_return_script.extend_from_slice(github_handle.as_bytes());

    let output = output_from_options(&op_return_script, AMT);

    // Compute change output script and output
    let change_value = COIN_VALUE - FEE;

    let change_output = output_from_options(&state.programs[0], change_value);

    // Get taproot spending data
    //   Compute destination output script
    let pubkey1 = state.pubs[0].x_only_public_key().0;
    let pubkey2 = state.pubs[1].x_only_public_key().0;
    let multisig = create_multisig_script(&[pubkey1, pubkey2]);

    //   Compute taptree root
    let taproot_root = get_taptree_root(&multisig);

    //   Compute witness program from taptree root and "NUMS" internal key
    let witness_program = get_p2tr_scriptpath_program(&get_nums(), &taproot_root);

    //   Determine if the output key (which is a tweaked internal key) has an odd Y-coordinate
    let parity_bit = get_p2tr_scriptpath_program_parity_bit(&get_nums(), &taproot_root);

    // Get the message to sign
    let mut outpoint_bytes = [0u8; 36];
    outpoint_bytes[..32].copy_from_slice(&outpoint.txid);
    outpoint_bytes[32..].copy_from_slice(&outpoint.vout.to_le_bytes());

    let message = get_commitment_hash(
        &outpoint_bytes,
        &witness_program,
        &COIN_VALUE,
        &[&output, &change_output],
        1,
        &Some(taproot_root),
    );

    // Sign!
    let witness =
        get_p2tr_scriptpath_witness(vec![state.privs[0], state.privs[1]], &message, parity_bit);

    // Assemble
    let assembled_tx =
        assemble_transaction(&[&spend_tx_input], &[&output, &change_output], &[&witness]);

    assembled_tx
}

// bitcoin-cli -signet testmempoolaccept '["020000000001015b260d2812ab56ff8eda6ea4278a569dcab4614799ab34932ddfcb4a5ef434acc301000000ffffffff0240420f00000000002251200e2ed2e6e95df8e1684c1ecb760331ec84e7f1fd8536346fafeec70a8e96eef973d1520000000000225120e8cf84318a4365882a5c93f4450a3eefad4c724eb2d177f1e13a087330fe65b30140e1713fdba8f69be31e07abd25dd343cd8a61bbb0f9a6ba9a628f5f1ab098cef0fbea53425d71a14516b928b5a26e97cda0257cdaa586ea2c9b761b1a8c5dc87d00000000"]'
