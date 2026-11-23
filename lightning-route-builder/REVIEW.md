# Code Review: submissions/src/main.rs

## Overview

Lightning Network route builder - well-structured with extracted functions. This review focuses on achieving idiomatic Rust with best practices.

---

## Critical Issues

### 1. Empty slice panic (lines 74, 88)

Both `calculate_htlc_amounts` and `calculate_cltv_expiries` will panic if `hops` is empty:

```rust
let last_index = hops.len() - 1;  // underflow panic if empty
```

**Fix:**
```rust
fn calculate_htlc_amounts(hops: &[Hop], final_amount: u64) -> Vec<u64> {
    if hops.is_empty() {
        return vec![];
    }
    // ...
}
```

### 2. Potential overflow in fee calculation (line 69)

```rust
amount_msat * hop.proportional_fee_ppm  // can overflow u64
```

For very large amounts, this multiplication can overflow. Consider:
```rust
fn calculate_fee(hop: &Hop, amount_msat: u64) -> u64 {
    hop.base_fee_msat
        .saturating_add(amount_msat.saturating_mul(hop.proportional_fee_ppm) / 1_000_000)
}
```

Or use checked arithmetic with proper error handling.

---

## Error Handling

### 3. All functions panic instead of returning `Result`

Every function uses `expect()` which causes panics. Idiomatic Rust uses `Result` for recoverable errors:

```rust
use anyhow::{Context, Result};

fn parse_args() -> Result<Args> {
    let mut args = std::env::args().skip(1);
    let output_dir: PathBuf = args
        .next()
        .context("Expected output directory")?
        .into();
    // ...
}

fn main() -> Result<()> {
    let args = parse_args()?;
    // ...
    Ok(())
}
```

---

## Type Safety

### 4. Unsafe `as` casts (lines 161, 162)

```rust
let amount_per_path = invoice_amount / num_paths as u64;
let min_final_cltv: u32 = invoice.min_final_cltv_expiry_delta() as u32;
```

Use `try_into()` for safe conversion:
```rust
let amount_per_path = invoice_amount / u64::try_from(num_paths)?;
let min_final_cltv: u32 = invoice.min_final_cltv_expiry_delta().try_into()?;
```

---

## Idiomatic Rust

### 5. Use doc comments instead of regular comments

```rust
// HTLC amount calculation for single path  // ← regular comment
fn calculate_htlc_amounts(...)
```

Should be:
```rust
/// Calculates HTLC amounts for each hop in the path, working backwards
/// from the final amount to determine fees.
fn calculate_htlc_amounts(...)
```

Apply to: `read_and_group_hops`, `calculate_htlc_amounts`, `calculate_cltv_expiries`, `process_path`

### 6. Add `#[must_use]` to pure functions

Functions that return values without side effects should have `#[must_use]`:

```rust
#[must_use]
fn calculate_fee(hop: &Hop, amount_msat: u64) -> u64 { ... }

#[must_use]
fn calculate_htlc_amounts(hops: &[Hop], final_amount: u64) -> Vec<u64> { ... }

#[must_use]
fn calculate_cltv_expiries(hops: &[Hop], height: u32, min_final_cltv: u32) -> Vec<u32> { ... }

#[must_use]
fn build_tlv_hex(payment_secret: &[u8; 32], total_msat: u64) -> String { ... }
```

### 7. Add `Debug` derive to `Args`

```rust
#[derive(Debug)]
struct Args { ... }
```

### 8. Unnecessary reference in `hex::encode` (line 112)

```rust
hex::encode(&tlv)  // & not needed
```

Should be:
```rust
hex::encode(tlv)
```

### 9. Explicit type annotations not needed (line 60)

```rust
let mut paths: BTreeMap<u32, Vec<Hop>> = BTreeMap::new();
```

Can be simplified to:
```rust
let mut paths = BTreeMap::new();
```

The type is inferred from usage.

### 10. Redundant comment (line 172)

```rust
// is mpp
let is_mpp = num_paths > 1;
```

The variable name is self-documenting. Remove the comment.

---

## Magic Numbers

### 11. Use named constants

```rust
const PPM_DIVISOR: u64 = 1_000_000;
const TLV_TYPE_PAYMENT_DATA: u64 = 8;
const TLV_LENGTH_PAYMENT_DATA: u64 = 40;
const TLV_CAPACITY: usize = 56;
const PAYMENT_SECRET_LEN: usize = 32;

fn calculate_fee(hop: &Hop, amount_msat: u64) -> u64 {
    hop.base_fee_msat + (amount_msat * hop.proportional_fee_ppm / PPM_DIVISOR)
}

fn build_tlv_hex(payment_secret: &[u8; PAYMENT_SECRET_LEN], total_msat: u64) -> String {
    let mut tlv = Vec::with_capacity(TLV_CAPACITY);
    tlv.extend_from_slice(&TLV_TYPE_PAYMENT_DATA.to_be_bytes());
    tlv.extend_from_slice(&TLV_LENGTH_PAYMENT_DATA.to_be_bytes());
    // ...
}
```

---

## Code Style

### 12. Grammar error in error message (line 37)

```rust
.expect("Expected a invoice")
```

Should be:
```rust
.expect("Expected an invoice")
```

### 13. `process_path` has too many parameters (8)

Consider grouping related parameters into a struct:

```rust
struct PathContext<'a> {
    height: u32,
    min_final_cltv: u32,
    payment_secret: &'a [u8; 32],
    total_invoice_amount: u64,
    is_mpp: bool,
}

fn process_path(
    path_id: u32,
    hops: &[Hop],
    amount: u64,
    ctx: &PathContext,
) -> Vec<OutputRow> { ... }
```

### 14. Recalculating `hops.len() - 1` in loop (line 132)

```rust
.map(|(i, hop)| {
    let is_last_hop = i == hops.len() - 1;  // calculated on each iteration
```

Cache it before the iterator:
```rust
let last_index = hops.len().saturating_sub(1);
hops.iter()
    .enumerate()
    .map(|(i, hop)| {
        let is_last_hop = i == last_index;
```

---

## Best Practices

### 15. Add clippy lints at crate level

Add to top of file:
```rust
#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::must_use_candidate)]  // if you don't want to add #[must_use] everywhere
```

### 16. Flush CSV writer explicitly

```rust
writer.flush().expect("Failed to flush CSV");
```

While `Writer` flushes on drop, explicit flushing with error handling is safer.

### 17. No unit tests

Add tests for pure functions:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_fee() {
        let hop = Hop {
            path_id: 0,
            channel_name: "test".to_string(),
            cltv_delta: 40,
            base_fee_msat: 1000,
            proportional_fee_ppm: 100,
        };
        assert_eq!(calculate_fee(&hop, 1_000_000), 1100);
    }

    #[test]
    fn test_calculate_fee_zero_amount() {
        let hop = Hop { base_fee_msat: 500, proportional_fee_ppm: 100, ..Default::default() };
        assert_eq!(calculate_fee(&hop, 0), 500);
    }

    #[test]
    fn test_build_tlv_hex() {
        let secret = [0u8; 32];
        let result = build_tlv_hex(&secret, 1000);
        assert_eq!(result.len(), 112);  // 56 bytes * 2 hex chars
    }
}
```

---

## Summary

| Category | Status | Priority |
|----------|--------|----------|
| Empty slice panic | Critical | High |
| Overflow in fee calc | Critical | High |
| Error handling (Result) | Poor | Medium |
| Type safety (as casts) | Poor | Medium |
| Doc comments | Missing | Low |
| Magic numbers | Present | Low |
| Unit tests | Missing | Medium |
| Clippy lints | Missing | Low |
