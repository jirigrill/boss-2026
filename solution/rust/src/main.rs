#![allow(warnings)]
use boss_wallet::balance::*;
use boss_wallet::spend::*;

///////////////////////////////
//   CHANGE THESE VALUES!     /
//  They were emailed to you  /
//    by an administrator     /
///////////////////////////////
const WALLET_NAME: &str = "wallet_151";
const EXTENDED_PRIVATE_KEY: &str = "tprv8ZgxMBicQKsPfK3no17Q4xi2gGr46o9Gfwjeo9hyEss3HMAeocT48doDqC79FFCdEG8VdsWBNr2CVj3wDhux4YX4fZbN9KDiy9YG5XEMHjc";

fn main() {
    let mut args = std::env::args().skip(1);
    let command = args.next().expect("Expected a command");

    let state = recover_wallet_state(EXTENDED_PRIVATE_KEY);

    // println!("Total UTXOs: {}", state.utxo.len());
    // println!();
    // for (outpoint_bytes, utxo) in &state.utxo {
    //     let outpoint = boss_wallet::balance::Outpoint::deserialize(outpoint_bytes);
    //     let mut txid_display = outpoint.txid;
    //     txid_display.reverse(); // Reverse for display (little-endian to big-endian)
    //     println!("TXID: {}", hex::encode(txid_display));
    //     println!("  Vout: {}", outpoint.vout);
    //     println!("  Value: {} BTC", utxo.value);
    //     println!("  ScriptPubKey: {}", hex::encode(utxo.scriptpubkey));
    //     println!();
    // }

    if command == "balance" {
        let mut bal = state.balance;
        bal.rescale(8);
        println!("{WALLET_NAME} {bal}");
    }

    if command == "spend" {
        let (txid, tx1) = spend_p2tr_keypath(&state);
        println!("{tx1}");
        // let tx2 = spend_p2tr_scriptpath(&state, &txid);
        //     println!("{tx2}");
        // Un-comment the line below to test your answer locally
        // Don't submit your code like that though or autograder will fail!
        // println!("{}", bcli(&format!("testmempoolaccept [\"{tx1}\",\"{tx2}\"]")));
    }
}
