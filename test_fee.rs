use rust_decimal::Decimal;

fn calculate_fee(vbytes: f64, fee_rate_btc_per_kvb: Decimal) -> Decimal {
    let fee_rate_vb = fee_rate_btc_per_kvb / Decimal::from(1000);
    let fee_btc = Decimal::from_f64_retain(vbytes).unwrap() * fee_rate_vb;

    let fee_sats = (fee_btc * Decimal::from(100_000_000)).ceil();
    fee_sats / Decimal::from(100_000_000)
}

fn main() {
    // Test with the failing case: 144 vB at 2.801 sat/vB
    let vbytes = 144.0;
    let fee_rate = Decimal::from_f64_retain(0.00002801).unwrap(); // 2.801 sat/vB in BTC/kvB

    let fee = calculate_fee(vbytes, fee_rate);
    let fee_sats = (fee * Decimal::from(100_000_000)).to_f64().unwrap();
    let actual_rate = fee_sats / vbytes;

    println!("vbytes: {}", vbytes);
    println!("target fee rate: 2.801 sat/vB");
    println!("fee in BTC: {}", fee);
    println!("fee in sats: {}", fee_sats);
    println!("actual rate: {} sat/vB", actual_rate);
    println!("meets requirement: {}", actual_rate >= 2.801);
}
