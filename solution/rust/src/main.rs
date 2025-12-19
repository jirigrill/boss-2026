use boss_wallet::balance::*;
use boss_wallet::spend::*;


///////////////////////////////
//   CHANGE THESE VALUES!     /
//  They were emailed to you  /
//    by an administrator     /
///////////////////////////////
const WALLET_NAME: &str = "wallet_076";
const EXTENDED_PRIVATE_KEY: &str = "tprv8ZgxMBicQKsPfF4wki3EzsSYvohmC5z2B92m74LscyiFAHwZUtn4Lhbfosc1zCQRzN4aXiPqiH64xbTbsnRCgTskJWspELwAYSdyZSekqyp";

fn main() {
    let mut args = std::env::args().skip(1);
    let command = args.next().expect("Expected a command");

    let state = recover_wallet_state(EXTENDED_PRIVATE_KEY);

    if command == "balance" {
        let mut bal = state.balance;
        bal.rescale(8);
        println!("{WALLET_NAME} {bal}");
    }

    if command == "spend" {
        let (txid, tx1) = spend_p2tr_keypath(&state);
        println!("{tx1}");
        let tx2 = spend_p2tr_scriptpath(&state, &txid);
        println!("{tx2}");
        // Un-comment the line below to test your answer locally
        // Don't submit your code like that though or autograder will fail!
        // println!("{}", bcli(&format!("testmempoolaccept [\"{tx1}\",\"{tx2}\"]")));
    }
}
