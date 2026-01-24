use std::collections::BTreeMap;

use lightning_invoice::Bolt11Invoice;

#[derive(Debug, serde::Deserialize)]
struct Hop {
    path_id: u32,
    channel_name: String,
    cltv_delta: u32,
    base_fee_msat: u64,
    proportional_fee_ppm: u64,
}

#[derive(Debug, serde::Serialize)]
struct OutputRow {
    path_id: u32,
    channel_name: String,
    htlc_amount_msat: u64,
    htlc_expiry: u32,
    tlv: String,
}

impl OutputRow {
    fn new(
        path_id: u32,
        channel_name: String,
        htlc_amount_msat: u64,
        htlc_expiry: u32,
        tlv: String,
    ) -> Self {
        Self {
            path_id,
            channel_name,
            htlc_amount_msat,
            htlc_expiry,
            tlv,
        }
    }
}

fn calculate_fee(hop: &Hop, amount_msat: u64) -> u64 {
    hop.base_fee_msat + (amount_msat * hop.proportional_fee_ppm / 1_000_000u64)
}

fn main() {
    // parse cli args
    let mut args = std::env::args().skip(1);
    let output_dir: String = args.next().expect("Expected a directory");
    let input_csv: String = args.next().expect("Expected a path");
    let invoice_str: String = args.next().expect("Expected a invoice");
    let height: u32 = args
        .next()
        .expect("Expected a height")
        .parse()
        .expect("Height must be valid u32");

    // read csv and data into vector of hops
    let mut reader = csv::Reader::from_path(input_csv).expect("Could not open CSV file");
    let hops: Vec<Hop> = reader
        .deserialize()
        .map(|r| r.expect("Failed to parse CSV row"))
        .collect();

    // group hops based on their path_id and count numner of paths
    let mut paths: BTreeMap<u32, Vec<Hop>> = BTreeMap::new();
    for hop in hops {
        paths.entry(hop.path_id).or_default().push(hop);
    }
    let num_paths = paths.len();

    // parse amount per path to pay in millisatoshis and minimum final cltv expire delta
    let invoice: Bolt11Invoice = invoice_str.parse().expect("Invalid payment request");
    let invoice_amount: u64 = invoice
        .amount_milli_satoshis()
        .expect("Invoice must have amount");
    let amount_per_path = invoice_amount / num_paths as u64;
    let min_final_cltv: u32 = invoice.min_final_cltv_expiry_delta() as u32;

    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_path(format!("{}/output.csv", output_dir))
        .expect("Could not write into CSV file");

    for (path_id, path_hops) in paths {
        // calculate HTLC amounts for each path
        let last_index = path_hops.len() - 1;
        let mut amounts = vec![0u64; path_hops.len()];
        amounts[last_index] = amount_per_path;
        for i in (0..last_index).rev() {
            let next_amount = amounts[i + 1];
            let fee = calculate_fee(&path_hops[i + 1], next_amount);
            amounts[i] = next_amount + fee;
        }

        // calculate cltv expires
        let mut expires = vec![0u32; path_hops.len()];
        expires[last_index] = height + min_final_cltv;
        for i in (0..last_index).rev() {
            expires[i] = expires[i + 1] + path_hops[i + 1].cltv_delta;
        }

        // // create OutputRows and write them into output
        for i in 0..path_hops.len() {
            writer
                .serialize(OutputRow::new(
                    path_id,
                    path_hops[i].channel_name.clone(),
                    amounts[i],
                    expires[i],
                    "NULL".to_string(),
                ))
                .expect("Filed to write row")
        }
    }
}
