# Plan: Lightning Payment Route Builder - Step 3 (TLV Values)

## What is TLV?

TLV (Type-Length-Value) is a flexible encoding format used in the Lightning protocol to extend message functionality. Each TLV record contains:

1. **Type** - Identifies what kind of data follows
2. **Length** - How many bytes the value occupies
3. **Value** - The actual data

For MPP payments, a `payment_data` TLV is included in the last hop of each path to tell the receiver:
- The payment secret (proves the payer knows the invoice)
- The total amount being paid across all HTLCs

---

## Key Changes from Step 2

### What's Different

1. **TLV field** - No longer always "NULL"; must be set for MPP last hops
2. **Payment secret extraction** - Need to get the 32-byte secret from the invoice
3. **Hex encoding** - TLV must be output as a hex string

### What Stays the Same

1. All fee and expiry calculations
2. Path grouping and amount splitting
3. Output format (just the TLV column changes)

---

## Section 1: When to Include TLV

### The Rule

- **Single-path payment** (num_paths == 1): TLV = "NULL" for all hops
- **Multi-path payment** (num_paths > 1): TLV = encoded value for **last hop of each path**, "NULL" for all other hops

### Why Only MPP?

The README explicitly states:
> *Do not include a MPP field* for inputs that only make a payment over a single path.

This simplifies grading across steps and differs from real-world implementations (which always include it nowadays).

---

## Section 2: TLV Structure

### The payment_data Record

The TLV for MPP payments has this structure:

| Field | Type | Size | Description |
|-------|------|------|-------------|
| type | uint64 | 8 bytes | Always 8 |
| length | uint64 | 8 bytes | Always 40 (0x28) |
| payment_secret | bytes | 32 bytes | From invoice |
| total_msat | uint64 | 8 bytes | Total invoice amount |

**Total size**: 8 + 8 + 32 + 8 = 56 bytes = 112 hex characters

### Byte Order

All integer fields use **big-endian** encoding (most significant byte first).

### Example

For payment_secret = `b3c3965128b05c96d76348158f8f3a1b92e2847172f9adebb400a9e83e62f066` and total_msat = 120:

```
0000000000000008    <- type (8 as uint64 big-endian)
0000000000000028    <- length (40 as uint64 big-endian)
b3c3965128b05c96d76348158f8f3a1b92e2847172f9adebb400a9e83e62f066  <- payment_secret (32 bytes)
0000000000000078    <- total_msat (120 = 0x78 as uint64 big-endian)
```

Full hex string:
```
00000000000000080000000000000028b3c3965128b05c96d76348158f8f3a1b92e2847172f9adebb400a9e83e62f0660000000000000078
```

---

## Section 3: Extracting Payment Secret from Invoice

### The Problem

The `lightning-invoice` crate provides access to the payment secret, but we need it as raw bytes for hex encoding.

### The Solution

```rust
let payment_secret: [u8; 32] = *invoice.payment_secret().as_ref();
```

Or using the `payment_secret()` method which returns a `PaymentSecret` type that can be converted to bytes.

### Important Detail

The payment secret is exactly 32 bytes. When hex-encoded, it becomes 64 characters.

---

## Section 4: Building the TLV Hex String

### The Problem

Need to construct a hex string from:
- Fixed type (8)
- Fixed length (40)
- Variable payment_secret (32 bytes from invoice)
- Variable total_msat (invoice amount)

### The Solution

```rust
fn build_tlv_hex(payment_secret: &[u8; 32], total_msat: u64) -> String {
    let mut tlv = Vec::with_capacity(56);

    // Type: 8 as uint64 big-endian
    tlv.extend_from_slice(&8u64.to_be_bytes());

    // Length: 40 as uint64 big-endian
    tlv.extend_from_slice(&40u64.to_be_bytes());

    // Payment secret: 32 bytes
    tlv.extend_from_slice(payment_secret);

    // Total msat: uint64 big-endian
    tlv.extend_from_slice(&total_msat.to_be_bytes());

    // Convert to hex string
    hex_encode(&tlv)
}
```

### Hex Encoding

Rust doesn't have built-in hex encoding. Options:

1. **Manual implementation** (no dependencies):
```rust
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
```

2. **Use `hex` crate** (add dependency):
```rust
hex::encode(&tlv)
```

Since the README says "must write your own code for... TLV encoding", the manual approach is safer.

---

## Section 5: Integrating TLV into Output

### The Problem

Need to determine when to output the TLV hex vs "NULL".

### The Solution

Track whether we're on the last hop of a path, and whether it's an MPP payment:

```rust
let is_mpp = num_paths > 1;

for (path_id, path_hops) in paths {
    // ... calculate amounts and expires ...

    for i in 0..path_hops.len() {
        let is_last_hop = i == path_hops.len() - 1;

        let tlv = if is_mpp && is_last_hop {
            build_tlv_hex(&payment_secret, invoice_amount)
        } else {
            "NULL".to_string()
        };

        writer.serialize(OutputRow::new(
            path_id,
            path_hops[i].channel_name.clone(),
            amounts[i],
            expires[i],
            tlv,
        ));
    }
}
```

### Important Detail

The `total_msat` in the TLV is the **total invoice amount**, NOT the per-path amount. All paths include the same total.

---

## Code Changes Required

### Changes to main.rs

1. **Extract payment secret** after parsing invoice:
```rust
let payment_secret: [u8; 32] = *invoice.payment_secret().as_ref();
```

2. **Add hex encoding function**:
```rust
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}
```

3. **Add TLV building function**:
```rust
fn build_tlv_hex(payment_secret: &[u8; 32], total_msat: u64) -> String {
    let mut tlv = Vec::with_capacity(56);
    tlv.extend_from_slice(&8u64.to_be_bytes());
    tlv.extend_from_slice(&40u64.to_be_bytes());
    tlv.extend_from_slice(payment_secret);
    tlv.extend_from_slice(&total_msat.to_be_bytes());
    hex_encode(&tlv)
}
```

4. **Determine if MPP** before the loop:
```rust
let is_mpp = num_paths > 1;
```

5. **Update output generation** to conditionally include TLV:
```rust
let is_last_hop = i == path_hops.len() - 1;
let tlv = if is_mpp && is_last_hop {
    build_tlv_hex(&payment_secret, invoice_amount)
} else {
    "NULL".to_string()
};
```

---

## Worked Example

### Input

```
path_id,channel_name,cltv_delta,base_fee_msat,proportional_fee_ppm
0,AliceBob,40,1000,10
0,BobCarol,65,2000,500
0,CarolDave,15,0,3000
1,AliceEve,20,5000,20
1,EveDave,40,15,2500
```

**Invoice**: 100,000 msat
**Payment secret**: `aabbccdd...` (32 bytes, 64 hex chars)
**Number of paths**: 2 (MPP payment)

### TLV Construction

```
Type:           0000000000000008
Length:         0000000000000028
Payment secret: aabbccdd... (64 hex chars)
Total msat:     00000000000186a0  (100,000 = 0x186a0)
```

### Output

```
0,AliceBob,52175,800098,NULL
0,BobCarol,50150,800033,NULL
0,CarolDave,50000,800018,00000000000000080000000000000028aabbccdd...00000000000186a0
1,AliceEve,50140,800058,NULL
1,EveDave,50000,800018,00000000000000080000000000000028aabbccdd...00000000000186a0
```

Note: Both last hops (CarolDave and EveDave) have the same TLV because they share the same payment secret and total amount.

---

## Single-Path Example (No TLV)

### Input

```
path_id,channel_name,cltv_delta,base_fee_msat,proportional_fee_ppm
0,AliceBob,40,1000,10
0,BobCarol,65,2000,500
0,CarolDave,15,0,3000
```

**Number of paths**: 1 (NOT MPP)

### Output

```
0,AliceBob,102350,800098,NULL
0,BobCarol,100300,800033,NULL
0,CarolDave,100000,800018,NULL
```

All TLV fields remain "NULL" because this is not an MPP payment.

---

## Backward Compatibility

The solution remains backward compatible:
- Step 1 inputs: Single path → no TLV → all "NULL" ✓
- Step 2 inputs: Multiple paths → TLV on last hops only ✓

---

## Dependencies

No new crate dependencies required. Hex encoding is implemented manually per README requirements:
> "must write your own code for... TLV encoding"

---

## Verification

1. Test with single-path inputs (Steps 1) - all TLV should be "NULL"
2. Test with multi-path inputs (Steps 2/3) - last hop of each path should have TLV
3. Verify TLV hex string is exactly 112 characters (56 bytes)
4. Verify total_msat in TLV equals invoice amount (not per-path amount)
5. Run `./test/check_run.sh`
