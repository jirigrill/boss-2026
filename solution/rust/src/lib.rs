use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, hash::Hash, ops::Add, process::Command};

const DUST_THRESHOLD: f64 = 0.00000546; //546 satoshis in BTC (Optional)

#[derive(Deserialize)]
pub struct Payment {
    pub address: String,
    #[serde(with = "rust_decimal::serde::float")]
    pub amount: Decimal,
    #[serde(with = "rust_decimal::serde::float")]
    pub rate: Decimal,
}

#[derive(Deserialize)]
struct Coin {
    // Will be read and written as JSON to and from bitcoin-cli
    // so no need to parse as bytes or reverse byte-order.
    pub txid: String,

    pub vout: u64,

    // Could also use scriptPubKey but it might be easier
    // to determine required spending conditions from an address.
    pub address: String,

    #[serde(with = "rust_decimal::serde::float")]
    pub amount: Decimal,
}

impl Coin {
    fn input_vbytes(&self) -> f64 {
        get_input_vbytes(&self.address)
    }

    fn is_segwit(&self) -> bool {
        AddressType::from_address(&self.address).is_segwit()
    }

    fn effective_value(&self, fee_rate_kvb: Decimal) -> Decimal {
        // Convert fee_rate from BTC/kvB to BTC/vB
        let fee_rate_vb = fee_rate_kvb / Decimal::from(1000);
        let input_cost = Decimal::from_f64_retain(self.input_vbytes()).unwrap() * fee_rate_vb;
        self.amount - input_cost
    }

    fn to_input_json(&self) -> serde_json::Value {
        serde_json::json!({
            "txid": self.txid,
            "vout": self.vout
        })
    }
}

fn get_input_vbytes(address: &str) -> f64 {
    AddressType::from_address(address).input_vbytes()
}

fn get_output_vbytes(address: &str) -> f64 {
    AddressType::from_address(address).output_vbytes()
}

fn calculate_fee(vbytes: f64, fee_rate_btc_per_kvb: Decimal) -> Decimal {
    // Convert fee rate from BTC/kvB to BTC/vB
    let fee_rate_vb = fee_rate_btc_per_kvb / Decimal::from(1000);
    let fee_btc = Decimal::from_f64_retain(vbytes).unwrap() * fee_rate_vb;

    // Round up to nearest satoshi to ensure we meet the minimum fee rate
    let fee_sats = (fee_btc * Decimal::from(100_000_000)).ceil();
    fee_sats / Decimal::from(100_000_000)
}

fn calculate_tx_vbytes(inputs: &[&Coin], has_change: bool, output_address: &str) -> f64 {
    // check if any input requires witness
    let has_witness = inputs.iter().any(|input| input.is_segwit());

    // base overhead
    let base = if has_witness { 10.5 } else { 10.0 };

    // sum input sizes
    let input_vbytes: f64 = inputs.iter().map(|input| input.input_vbytes()).sum();

    // payment output
    let output_vbytes = get_output_vbytes(&output_address);

    let mut total = base + input_vbytes + output_vbytes;

    if has_change {
        total += 31.0; // change_vbytes
    }

    total
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddressType {
    P2TR,   // Taproot
    P2WPKH, // Native SegWit
    P2SH,   // Nested SegWit
    P2PKH,  // Legacy
}

impl AddressType {
    fn from_address(address: &str) -> Self {
        if address.starts_with("bcrt1p") || address.starts_with("bc1p") {
            AddressType::P2TR
        } else if address.starts_with("bcrt1q") || address.starts_with("bc1q") {
            AddressType::P2WPKH
        } else if address.starts_with("2") || address.starts_with("3") {
            AddressType::P2SH
        } else {
            AddressType::P2PKH
        }
    }

    fn input_vbytes(&self) -> f64 {
        match self {
            AddressType::P2TR => 57.5,
            AddressType::P2WPKH => 68.0,
            AddressType::P2SH => 91.0,
            AddressType::P2PKH => 148.0,
        }
    }

    fn output_vbytes(&self) -> f64 {
        match self {
            AddressType::P2TR => 43.0,
            AddressType::P2WPKH => 31.0,
            AddressType::P2SH => 32.0,
            AddressType::P2PKH => 34.0,
        }
    }

    fn is_segwit(&self) -> bool {
        matches!(
            self,
            AddressType::P2TR | AddressType::P2WPKH | AddressType::P2SH
        )
    }
}

fn format_inputs(selected: &[&Coin]) -> String {
    let inputs: Vec<_> = selected.iter().map(|coin| coin.to_input_json()).collect();
    serde_json::to_string(&inputs).unwrap()
}

fn format_outputs(outputs: HashMap<String, Decimal>) -> String {
    let outputs_map: HashMap<String, f64> = outputs
        .iter()
        .map(|(addr, amount)| {
            // Round to 8 decimal places (satoshi precision)
            let rounded = amount.round_dp(8);
            (addr.clone(), rounded.to_string().parse::<f64>().unwrap())
        })
        .collect();
    serde_json::to_string(&outputs_map).unwrap()
}

fn parse_hex(signed_result: &str) -> String {
    let signed_json: Value = serde_json::from_str(signed_result).unwrap();
    signed_json["hex"].as_str().unwrap().to_string()
}

pub fn bcli(cmd: &str) -> String {
    let res = Command::new("bitcoin-cli")
        .arg("-regtest")
        .arg("-datadir=/Users/jirigrill/Developer/boss-2026/coin-selection/datadir/node0")
        .arg("-rpcuser=student")
        .arg("-rpcpassword=boss2026")
        .arg("-rpcwallet=student")
        .args(cmd.split_whitespace())
        .output()
        .expect("bitcoin-cli command failed");

    let stdout = String::from_utf8_lossy(&res.stdout).to_string();
    let stderr = String::from_utf8_lossy(&res.stderr).to_string();

    if !stderr.is_empty() {
        eprintln!("bitcoin-cli stderr: {}", stderr);
    }

    if stdout.trim().is_empty() && !stderr.is_empty() {
        eprintln!("ERROR: bitcoin-cli command '{}' returned empty stdout", cmd);
        eprintln!("stderr was: {}", stderr);
        panic!("bitcoin-cli failed");
    }

    stdout
}

fn select_coins_greedy<'a>(utxos: &'a [Coin], payment: &'a Payment) -> Vec<&'a Coin> {
    let fee_rate = payment.rate;
    let target = payment.amount;

    // filter dust and sort by effective value descending
    let mut viable_utxos: Vec<&Coin> = utxos
        .iter()
        .filter(|utxo| utxo.effective_value(fee_rate) > Decimal::ZERO)
        .collect();
    viable_utxos.sort_by(|a, b| {
        b.effective_value(fee_rate)
            .cmp(&a.effective_value(fee_rate))
    });

    eprintln!(
        "Greedy selection: {} total UTXOs, {} viable",
        utxos.len(),
        viable_utxos.len()
    );
    eprintln!("Target: {} BTC, Fee rate: {} BTC/kvB", target, fee_rate);

    let mut selected = Vec::new();
    let mut total_value: Decimal = Decimal::ZERO;

    for utxo in viable_utxos {
        selected.push(utxo);
        total_value += utxo.amount;

        // Calculate fee with change output
        let tx_vbytes = calculate_tx_vbytes(&selected, true, &payment.address);
        let fee = calculate_fee(tx_vbytes, fee_rate);
        let required = target + fee;

        eprintln!(
            "  Selected {} UTXOs, total_value={}, required={}",
            selected.len(),
            total_value,
            required
        );

        if total_value >= required {
            return selected;
        }
    }
    eprintln!("WARNING: Greedy selection exhausted all UTXOs but didn't meet requirement!");
    selected
}

pub fn make_payments(payments: &[Payment]) {
    // Make payments
    for payment in payments {
        // Get all UTXO in wallet coin pool
        let list_unspent = bcli("listunspent");
        // Objectify coins to spend
        let coins: Vec<Coin> = serde_json::from_str(&list_unspent).unwrap();

        // Select coins using greedy algorithm
        let selected = select_coins_greedy(&coins, payment);

        if selected.is_empty() {
            eprintln!("ERROR: Coin selection failed!");
            eprintln!("Available UTXOs: {}", coins.len());
            eprintln!(
                "Payment: amount={}, rate={} BTC/kvB, address={}",
                payment.amount, payment.rate, payment.address
            );
            for coin in &coins {
                let eff_val = coin.effective_value(payment.rate);
                eprintln!(
                    "  UTXO: amount={}, address={}, eff_value={}",
                    coin.amount, coin.address, eff_val
                );
            }
            panic!("No suitable UTXOs found for payment");
        }

        // Calculate change
        let total_input: Decimal = selected.iter().map(|selected| selected.amount).sum();
        let tx_vbytes = calculate_tx_vbytes(&selected, true, &payment.address);
        let fee = calculate_fee(tx_vbytes, payment.rate);
        let change_amount = total_input - payment.amount - fee;

        let mut outputs = HashMap::new();
        outputs.insert(payment.address.clone(), payment.amount);
        // decide if we create change output
        if change_amount > Decimal::from_f64_retain(DUST_THRESHOLD).unwrap() {
            // create change output
            let change_address = bcli(&format!("getrawchangeaddress bech32"))
                .trim()
                .to_string(); // P2WPKH
            outputs.insert(change_address, change_amount);
        }

        //create and broadcast transaction
        let inputs_json = format_inputs(&selected);
        let outputs_json = format_outputs(outputs);

        let unsigned_tx = bcli(&format!(
            "createrawtransaction {inputs_json} {outputs_json}"
        ))
        .trim()
        .to_string();
        // Sign transaction
        let signed_result = bcli(&format!("signrawtransactionwithwallet {unsigned_tx}"));
        let signed_hex = parse_hex(&signed_result.trim());

        // Broadcast transaction without maxfeerate protection
        bcli(&format!("sendrawtransaction {signed_hex} 0"));

        // Confirm transaciton
        bcli("generatetoaddress 1 bcrt1pqqqqrldkrl");
    }
}
