#!/usr/bin/env python3

# Python wrapper for libscep256k1, the cryptography library used
# by Bitcoin Core. This wrapper is maintained by well-known contributors:
# https://github.com/rustyrussell/secp256k1-py
from secp256k1 import PrivateKey, PublicKey

import hashlib
from typing import List
from balance import (
    EXTENDED_PRIVATE_KEY,
    bcli,
    get_pub_from_priv,
    recover_wallet_state,
    tagged_hash
)


# Nothing-up-my-sleeve point from BIP 341.
# Used as a taproot internal key to effectively disable key-path spends, forcing a script-path spend.
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs
NUMS_PUBKEY = PublicKey(bytes.fromhex("0250929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0"), raw=True)


# If the public key derived from a private key has an odd-Y value instead of
# an even-Y value, we need to negate the private key, which will result in
# the required even-Y public key.
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#constructing-and-spending-taproot-outputs
# It would be nice if this method was included in the secp256k1 python wrapper
# but it is not, so we will give it to you for free ;-)
def maybe_negate_priv(priv: PrivateKey) -> PrivateKey:
    CURVE_ORDER = 0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141
    parity_byte = priv.pubkey.serialize()[0]
    if parity_byte == 0x02:
        return priv
    else:
        negated_priv_bytes = (CURVE_ORDER - int.from_bytes(priv.private_key)).to_bytes(32)
        return PrivateKey(negated_priv_bytes)


# Given 2 secp256k1 PublicKey objects, construct a 2-of-2 multisig taproot script.
# Return raw bytes of the script (without a length byte prefix)
def create_multisig_script(keys: List[PublicKey]) -> bytes:
    # Get the 32-byte X coordinate for each key



# Given a single taproot script as a byte array, compute the taptree root hash.
def get_taptree_root(script: bytes) -> bytes:



# Given a secp256k1 PublicKey object and a taptree root as bytes, construct the witness program as bytes.
# This is a SegWit version 1 pay-to-taproot witness program:
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
def get_p2tr_scriptpath_program(internal_key: PublicKey, taptree_root: bytes) -> bytes:
    # Get the 32-byte X coordinate from the internal public key

    # Compute the TapTweak

    # Compute the output public key by tweaking the internal key

    # Compile the version 1 witness program
    # https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki#user-content-Witness_program



# Given a secp256k1 PublicKey object and a taptree root as bytes, determine if the
# Y coordinate of the output key (which is a tweaked internal key) is odd.
# This bit is required in the first byte of the control block in the witness.
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-10
def get_p2tr_scriptpath_program_parity_bit(internal_key: PublicKey, taptree_root: bytes) -> bool:
    # Get the 32-byte X coordinate from the internal public key

    # Compute the TapTweak

    # Compute the output public key by tweaking the internal key

    # Return the public key Y coordinate parity



# Given an outpoint, return a serialized transaction input spending it.
# Use hard-coded defaults for sequence and scriptSig.
def input_from_utxo(txid: bytes, index: int) -> bytes:



# Given an output script and value (in satoshis), return a serialized transaction output.
def output_from_options(script: bytes, value: int) -> bytes:



# Compute the commitment hash for a single input and return bytes to sign.
# This implements the BIP 341 transaction digest algorithm
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#common-signature-message
# We assume only a single input and two outputs, SIGHASH_DEFAULT (which aka SIGHASH_ALL)
# as well as constant default values for sequence and locktime
def get_commitment_hash(outpoint: bytes, scriptpubkey: bytes, value: int, outputs: List[bytes], ext_flag: bytes, taptree_root=None) -> bytes:
    def sha256(data: bytes) -> bytes:
        return hashlib.new("sha256", data).digest()

    # Initialize serialization
    ser = bytes()
    # Epoch
    # https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#cite_note-20

    # Control
    #   Hash Type (SIGHASH_DEFAULT aka SIGHASH_ALL)

    # Transaction data
    #   Version, default 2

    #   Locktime, default 0

    #   SHA256 of the serialization of all input outpoints (only one in our case)

    #   SHA256 of the serialization of all input amounts (only one in our case)

    #   SHA256 of all spent outputs' scriptPubKeys including length prefix (only one in our case)

    #   SHA256 of the serialization of all input nSequence, default 0xffffffff (only one in our case)

    #   SHA256 of the serialization of all outputs including length prefix

    # Data about this input
    #   Spend type (0 for key-path spend, 2 for script-path spend)

    #   index of this input in the transaction input vector

    # Data about this output
    #   (nothing needed here for key-path since we are signing SIGHASH_ALL)
    #   For script-path spends, extend the message
    #   https://github.com/bitcoin/bips/blob/master/bip-0342.mediawiki#common-signature-message-extension

        # The tapleaf hash as defined in BIP341
        # Since we only have one script in our tree the leaf hash *is* the tree root hash

        # A constant value 0x00 representing the current version of public keys in the tapscript signature opcode execution.

        # The opcode position of the last executed OP_CODESEPARATOR (or 0xffffffff if none executed)

    # Return the TapSighash-tagged hash of the aboce serialization



# Given a JSON utxo object and a list of all of our wallet's witness programs,
# return the index of the derived key that can spend the coin.
# This index should match the corresponding private key in our wallet's list.
def get_key_index(utxo: object, programs: List[str]) -> int:



# Given a secp256k1 PrivateKey object and TapSighash message as bytes, compute the BIP 340 Schnorr signature.
def sign(priv: PrivateKey, msg: bytes) -> bytes:



# Given a secp256k1 PrivateKey object and TapSighash message to sign,
# compute the signature and assemble the serialized p2tr key-path witness
# as defined in BIP 341 (1 stack item only: signature)
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
def get_p2tr_keypath_witness(priv: PrivateKey, msg: bytes) -> bytes:



# Given two secp256k1 PrivateKey objects and a transaction commitment hash to sign,
# compute both signatures and assemble the serialized p2tr script-path witness
# as defined in BIP 341 (4 stack items: signature0, signature1, script, control block)
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
def get_p2tr_scriptpath_witness(privs: List[PrivateKey], msg: bytes, parity_bit: bool) -> bytes:
    # Push number of witness items

    # Push signatures into witness in reverse order because they will be un-reversed on the stack

    # Derive script and push bytes to witness

    # Because we only have one script in our taptree, the control block
    # is just the prefix byte plus the internal key, which is our NUMS point.



# Given arrays of inputs, outputs, and witnesses, assemble the complete
# transaction and serialize it for broadcast. Return bytes as hex-encoded string
# suitable to broadcast with Bitcoin Core RPC.
# https://en.bitcoin.it/wiki/Protocol_documentation#tx
def assemble_transaction(inputs: List[bytes], outputs: List[bytes], witnesses: List[bytes]) -> str:



# Given arrays of inputs and outputs (no witnesses!) compute the txid.
# Return the 32 byte txid as a *reversed* hex-encoded string.
# https://developer.bitcoin.org/reference/transactions.html#raw-transaction-format
def get_txid(inputs: List[bytes], outputs: List[bytes]) -> str:



# Create a transaction that spends a p2tr key-path utxo to a 2-of-2 multisig
# p2tr script-path program and return both the txid and the final transaction as hex strings
def spend_p2tr_keypath(state: object) -> (str, str):
    FEE = 1000
    AMT = 1000000
    # Choose an unspent coin worth more than 0.01 BTC

    # Create the input from the utxo
    # Reverse the txid hash from human-readable JSON format to bitcoin wire format

    # Compute destination output script

    # Compute taptree root

    # Compute witness program from taptree root and "NUMS" internal key

    # Get a change output script (one of our wallet's witness programs) and create a change output

    # Get the message to sign

    # Fetch the private key we need to sign with

    # Might need to negate this private key before continuing

    # Tweak the private key according to
    # https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki#tr

    # Sign!

    # Assemble

    # Reserialize without witness data and double-SHA256 to get the txid



# Create a transaction that spends a 2-of-2 multisig p2tr script-path utxo to a
# an OP_RETURN output that contains YOUR GITHUB HANDLE as a string!
# Return the final transaction as a hex string.
def spend_p2tr_scriptpath(state: object, txid: str) -> str:
    COIN_VALUE = 1000000
    FEE = 1000
    AMT = 0
    # Create the input from the utxo
    # Reverse the txid hash from human-readable JSON format to bitcoin wire format

    # Compute destination output script and output

    # Compute change output script and output

    # Get taproot spending data
    #   Compute destination output script

    #   Compute taptree root

    #   Compute witness program from taptree root and "NUMS" internal key

    #   Determine if the output key (which is a tweaked internal key) has an odd Y-coordinate

    # Get the message to sign

    # Sign!

    # Assemble



if __name__ == "__main__":
    # Recover wallet state: We will need all key pairs and unspent coins
    state = recover_wallet_state(EXTENDED_PRIVATE_KEY)
    txid1, tx1 = spend_p2tr_keypath(state)
    print(tx1)
    tx2 = spend_p2tr_scriptpath(state, txid1)
    print(tx2)
    # Un-comment the line below to test your answer locally
    # Don't submit your code like that though or autograder will fail!
    # print(bcli(f"testmempoolaccept [\"{tx1}\",\"{tx2}\"]"))
