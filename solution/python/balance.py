#!/usr/bin/env python3

# Python wrapper for libscep256k1, the cryptography library used
# by Bitcoin Core. This wrapper is maintained by well-known contributors:
# https://github.com/rustyrussell/secp256k1-py
from secp256k1 import PrivateKey, PublicKey

from decimal import Decimal
from subprocess import run
from typing import List, Tuple
import hashlib
import hmac
import json

##############################
#   CHANGE THESE VALUES!     #
#  They were emailed to you  #
#    by an administrator     #
##############################
WALLET_NAME = "wallet_076"
EXTENDED_PRIVATE_KEY = "tprv8ZgxMBicQKsPfF4wki3EzsSYvohmC5z2B92m74LscyiFAHwZUtn4Lhbfosc1zCQRzN4aXiPqiH64xbTbsnRCgTskJWspELwAYSdyZSekqyp"



# Assuming Bitcoin Core is running and connected to signet using default datadir,
# execute an RPC and return its value or error message.
# https://github.com/bitcoin/bitcoin/blob/master/doc/bitcoin-conf.md#configuration-file-path
# Examples: bcli("getblockcount")
#           bcli("getblockhash 100")
def bcli(cmd: str):
    res = run(
            ["bitcoin-cli", "-signet"] + cmd.split(" "),
            capture_output=True,
            encoding="utf-8")
    if res.returncode == 0:
        return res.stdout.strip()
    else:
        raise Exception(res.stderr.strip())


# Decode a base58 string into an array of bytes
def base58_decode(base58_string: str) -> bytes:
    base58_alphabet = "123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz"
    # Convert Base58 string to a big integer

    # Convert the integer to bytes

    # Chop off the 32 checksum bits and return
    # BONUS POINTS: Verify the checksum!



# Deserialize the extended key bytes and return a JSON object
# https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki#serialization-format
# 4 byte: version bytes (mainnet: 0x0488B21E public, 0x0488ADE4 private; testnet: 0x043587CF public, 0x04358394 private)
# 1 byte: depth: 0x00 for master nodes, 0x01 for level-1 derived keys, ....
# 4 bytes: the fingerprint of the parent's key (0x00000000 if master key)
# 4 bytes: child number. This is ser32(i) for i in xi = xpar/i, with xi the key being serialized. (0x00000000 if master key)
# 32 bytes: the chain code
# 33 bytes: the public key or private key data (serP(K) for public keys, 0x00 || ser256(k) for private keys)
def deserialize_key(b: bytes) -> object:



# Compute a tagged hash as defined in
# https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki#design
def tagged_hash(data: bytes, tag: str) -> bytes:



# Derive the secp256k1 compressed public key from a given private key
# BONUS POINTS: Implement secp256k1 yourself and multiply your key by the generator point!
def get_pub_from_priv(priv: PrivateKey) -> PublicKey:



# Perform a BIP32 parent private key -> child private key operation
# Return a JSON object with "key" and "chaincode" properties as bytes
# https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki#user-content-Private_parent_key_rarr_private_child_key
def derive_priv_child(key: PrivateKey, chaincode: bytes, index: int, hardened: bool) -> object:



# Given a master secp256k1 PrivateKey object, a chaincode as bytes, and a BIP32 derivation path,
# compute the first 2000 child private keys.
# Return an array of keys encoded as secp256k1 PrivateKey objects.
# The derivation path is formatted as an array of (index: int, hardened: bool) tuples.
def get_wallet_privs(key: PrivateKey, chaincode: bytes, path: List[Tuple[int, bool]]) -> List[PrivateKey]:
    out = []
    # Derive the account key from the path

    return out


# Derive the p2tr witness program for a given public key as a secp256k1 PublicKey object.
# Return a bytes array to be compared with the ScriptPubKey in transaction outputs
# in the JSON response from Bitcoin Core RPC getblock so we can determine
# if the transaction output is "received" by our wallet.
# These are SegWit version 1 pay-to-taproot witness programs.
# https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki#script-validation-rules
def get_p2tr_keypath_program(pubkey: PublicKey) -> bytes:
    # Get the 32-byte X coordinate from the public key

    # Compute the TapTweak

    # Make sure the internal public key has an even Y coordinate

    # Compute the output public key by tweaking the internal key
    # https://github.com/bitcoin/bips/blob/master/bip-0386.mediawiki#tr

    # Compile the version 1 witness program
    # https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki#user-content-Witness_program



# Recover the wallet state from the blockchain:
# - Parse xprv and path from descriptor and derive 2000 key pairs and witness programs
# - Request blocks 0-300 from Bitcoin Core via RPC and scan all transactions
# - Return a state object with all the derived keys and total wallet balance
def recover_wallet_state(xprv: str) -> object:
    # Generate all the keypairs and witness programs to search for


    # Prepare a wallet state data structure
    state = {
        "utxo": {},
        "balance": 0,
        "privs": privs,
        "pubs": pubs,
        "programs": programs
    }

    # Scan blocks 0-300

        # Scan every tx in every block

            # Check every tx input for our own utxos, by their outpoint
            # These are coins we have spent.

                    # Remove this coin from our wallet state utxo pool
                    # so we don't double spend it later

            # Check every tx output for our own witness programs.
            # These are coins we have received.

                    # Add to our total balance

                    # Keep track of this UTXO by its outpoint in case we spend it later

    return state


if __name__ == "__main__":
    state = recover_wallet_state(EXTENDED_PRIVATE_KEY)
    print(f"{WALLET_NAME} {state['balance']}")

