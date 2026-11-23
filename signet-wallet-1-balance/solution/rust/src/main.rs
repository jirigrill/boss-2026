use boss_wallet::balance::*;
use hmac::digest::consts::True;
use secp256k1::SecretKey;

///////////////////////////////
//   CHANGE THESE VALUES!     /
//  They were emailed to you  /
//    by an administrator     /
///////////////////////////////
const WALLET_NAME: &str = "wallet_151";
const EXTENDED_PRIVATE_KEY: &str = "tprv8ZgxMBicQKsPfK3no17Q4xi2gGr46o9Gfwjeo9hyEss3HMAeocT48doDqC79FFCdEG8VdsWBNr2CVj3wDhux4YX4fZbN9KDiy9YG5XEMHjc";

fn main() {
    // let decoded_bytes = base58_decode(EXTENDED_PRIVATE_KEY);
    // println!("decoded private key: {}", hex::encode(&decoded_bytes));
    // let deserialized_key = deserialize_key(&decoded_bytes);
    // println!("{}", deserialized_key);
    // let tagged_hash_test = tagged_hash(&decoded_bytes, "BIP0340/aux");
    // println!("tagged hash test: {}", hex::encode(tagged_hash_test));

    // let key_bytes: [u8; 32] = deserialized_key.key[1..].try_into().unwrap();
    // println!("secret_key: {}", hex::encode(key_bytes));
    // let secret_key = SecretKey::from_byte_array(key_bytes).unwrap();

    // let public_key = get_pub_from_priv(&secret_key);
    // println!("public key: {}", public_key);

    // let der_priv_child_key = derive_priv_child(&secret_key, &deserialized_key.chaincode, 0, false);

    // println!("{:?}", hex::encode(der_priv_child_key.key.secret_bytes()));

    let state = recover_wallet_state(EXTENDED_PRIVATE_KEY);

    // let mut bal = state.balance;
    // bal.rescale(8);
    // println!("{WALLET_NAME} {bal}");

    let mut args = std::env::args().skip(1);
    let command = args.next().expect("Expected a command");

    if command == "balance" {
        let mut bal = state.balance;
        bal.rescale(8);
        println!("{WALLET_NAME} {bal}");
    }
}
