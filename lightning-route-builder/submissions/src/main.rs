#![warn(clippy::all, clippy::pedantic)]

use std::{
    collections::BTreeMap,
    error::Error,
    path::{Path, PathBuf},
};

use lightning_invoice::Bolt11Invoice;

#[derive(Debug)]
struct Args {
    output_dir: PathBuf,
    input_csv: PathBuf,
    invoice: String,
    height: u32,
}

struct PathContext<'a> {
    height: u32,
    min_final_cltv: u32,
    payment_secret: &'a [u8; 32],
    invoice_amount: u64,
    is_mpp: bool,
}

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

const PPM_DIVISOR: u64 = 1_000_000;
const TLV_TYPE_PAYMENT_DATA: u64 = 8;
const TLV_LENGTH_PAYMENT_DATA: u64 = 40;
const TLV_CAPACITY: usize = 56;
const PAYMENT_SECRET_LEN: usize = 32;

fn parse_args() -> Result<Args, Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let output_dir: PathBuf = args.next().ok_or("Expected a directory")?.into();
    let input_csv: PathBuf = args.next().ok_or("Expected a path")?.into();
    let invoice_str: String = args.next().ok_or("Expected ani9k invoice")?;
    let height: u32 = args.next().ok_or("Expected a height")?.parse()?;

    Ok(Args {
        output_dir,
        input_csv,
        invoice: invoice_str,
        height,
    })
}

// CSV reading and hop grouping
fn read_and_group_hops(path: &Path) -> BTreeMap<u32, Vec<Hop>> {
    let mut reader = csv::Reader::from_path(path).expect("Could not open CSV file");
    let hops: Vec<Hop> = reader
        .deserialize()
        .map(|r| r.expect("Failed to parse CSV row"))
        .collect();

    let mut paths: BTreeMap<u32, Vec<Hop>> = BTreeMap::new();
    for hop in hops {
        paths.entry(hop.path_id).or_default().push(hop);
    }

    paths
}

fn calculate_fee(hop: &Hop, amount_msat: u64) -> u64 {
    hop.base_fee_msat + (amount_msat * hop.proportional_fee_ppm / PPM_DIVISOR)
}

// HTLC amount calculation for single path
fn calculate_htlc_amounts(hops: &[Hop], final_amount: u64) -> Vec<u64> {
    let last_index = hops.len() - 1;
    let mut amounts = vec![0u64; hops.len()];
    amounts[last_index] = final_amount;
    for i in (0..last_index).rev() {
        let next_amount = amounts[i + 1];
        let fee = calculate_fee(&hops[i + 1], next_amount);
        amounts[i] = next_amount + fee;
    }

    amounts
}

// CLTV expiry calculation for single path
fn calculate_cltv_expiries(hops: &[Hop], height: u32, min_final_cltv: u32) -> Vec<u32> {
    let last_index = hops.len() - 1;
    let mut expires = vec![0u32; hops.len()];
    expires[last_index] = height + min_final_cltv;
    for i in (0..last_index).rev() {
        expires[i] = expires[i + 1] + hops[i + 1].cltv_delta;
    }
    expires
}

fn build_tlv_hex(payment_secret: &[u8; PAYMENT_SECRET_LEN], total_msat: u64) -> String {
    let mut tlv = Vec::with_capacity(TLV_CAPACITY);

    // type: 8 as uint64 big-endian
    tlv.extend_from_slice(&TLV_TYPE_PAYMENT_DATA.to_be_bytes());

    // length: 40 as uint64 big-endian
    tlv.extend_from_slice(&TLV_LENGTH_PAYMENT_DATA.to_be_bytes());

    // payment secret: 32 bytes
    tlv.extend_from_slice(payment_secret);

    // total msat: uint64 big-endian
    tlv.extend_from_slice(&total_msat.to_be_bytes());

    hex::encode(tlv)
}

// Process a single path and return output rows
fn process_path(path_id: u32, hops: &[Hop], amount: u64, ctx: &PathContext) -> Vec<OutputRow> {
    let amounts = calculate_htlc_amounts(hops, amount);
    let expires = calculate_cltv_expiries(hops, ctx.height, ctx.min_final_cltv);

    let last_index = hops.len().saturating_sub(1);
    hops.iter()
        .enumerate()
        .map(|(i, hop)| {
            let is_last_hop = i == last_index;
            let tlv = if ctx.is_mpp && is_last_hop {
                build_tlv_hex(ctx.payment_secret, ctx.invoice_amount)
            } else {
                "NULL".to_string()
            };

            OutputRow {
                path_id,
                channel_name: hop.channel_name.clone(),
                htlc_amount_msat: amounts[i],
                htlc_expiry: expires[i],
                tlv,
            }
        })
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;

    let paths = read_and_group_hops(&args.input_csv);
    let invoice: Bolt11Invoice = args.invoice.parse().expect("Invalid payment request");
    let num_paths = paths.len();

    // parse amount per path to pay in millisatoshis and minimum final cltv expire delta
    let invoice_amount: u64 = invoice
        .amount_milli_satoshis()
        .expect("Invoice must have amount");
    let amount_per_path = invoice_amount / u64::try_from(num_paths)?;
    let min_final_cltv: u32 = invoice.min_final_cltv_expiry_delta().try_into()?;

    // parse payment_secret for tlv
    let payment_secret: [u8; 32] = invoice.payment_secret().0;

    let mut writer = csv::WriterBuilder::new()
        .has_headers(false)
        .from_path(args.output_dir.join("output.csv"))
        .expect("Could not write into CSV file");

    let is_mpp = num_paths > 1;

    let ctx = PathContext {
        height: args.height,
        min_final_cltv,
        payment_secret: &payment_secret,
        invoice_amount,
        is_mpp,
    };

    for (path_id, hops) in paths {
        let rows = process_path(path_id, &hops, amount_per_path, &ctx);

        for row in rows {
            writer.serialize(row).expect("Failed to write row");
        }
    }
    writer.flush().expect("Failed to flush CSV");
    Ok(())
}
