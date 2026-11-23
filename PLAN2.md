# Plan: Lightning Payment Route Builder - Step 2 (MPP Route)

## What is MPP (Multi-Part Payment)?

Multi-Part Payment (MPP) allows splitting a single payment across multiple routes. Instead of sending the full amount through one path, the payment is divided into smaller pieces that travel different paths through the network. This is useful because:

1. **Liquidity constraints** - A single channel may not have enough capacity for the full payment
2. **Reliability** - If one path fails, only that portion needs to be retried
3. **Privacy** - Routing nodes see smaller amounts

The receiver collects all HTLCs and only releases the preimage once the full amount arrives.

---

## Key Changes from Step 1

### What's Different

1. **Multiple paths** - Input CSV now contains hops for multiple distinct routes, identified by `path_id`
2. **Amount splitting** - The invoice amount is split equally across all paths
3. **Independent calculations** - Each path has its own fee and expiry calculations

### What Stays the Same

1. Fee calculation formula (BOLT7)
2. Backward calculation approach (from receiver to sender)
3. CLTV delta logic
4. TLV field remains "NULL" (that's for Step 3)

---

## Section 1: Grouping Hops by Path

### The Problem

In Step 1, all hops belonged to a single path (path_id = 0). Now hops can have different path_ids, and we need to process each path independently.

### The Solution

After reading the CSV:
1. Group hops by their `path_id` field
2. Maintain the original order within each group (hops arrive in route order)
3. Count the number of unique paths

### Pseudocode

```
hops = read_csv(input_file)

paths = HashMap<path_id, Vec<Hop>>

for hop in hops:
    paths[hop.path_id].append(hop)

num_paths = paths.len()
```

### Important Detail

Hops in the CSV are already ordered correctly within each path. The input might interleave paths (all of path 0, then all of path 1, etc.) but within each path_id, hops are in route order from sender to receiver.

---

## Section 2: Splitting the Payment Amount

### The Problem

The invoice specifies a total amount (e.g., 120,000 msat). This must be divided equally among all paths.

### The Solution

```
amount_per_path = invoice_amount / num_paths
```

The problem statement guarantees the amount is always divisible by the number of paths, so no remainder handling is needed.

### Example

- Invoice: 120,000 msat
- Paths: 3
- Each path delivers: 40,000 msat to the receiver

---

## Section 3: Calculating Amounts Per Path

### The Problem

Each path must deliver `amount_per_path` to the receiver. Fees accumulate as we work backwards, but each path has its own fee structure.

### The Solution

Apply the same backward calculation from Step 1, but independently for each path:

```
for each path in paths:
    hops = path.hops
    last_index = hops.len() - 1

    amounts[last_index] = amount_per_path  # NOT invoice_amount!

    for i from (last_index - 1) down to 0:
        next_amount = amounts[i + 1]
        fee = calculate_fee(hops[i + 1], next_amount)
        amounts[i] = next_amount + fee
```

### Key Insight

The only change from Step 1 is using `amount_per_path` instead of `invoice_amount` for the final hop. Everything else is identical.

---

## Section 4: Calculating CLTV Expiries Per Path

### The Problem

Each path needs its own expiry calculations. Different paths may have different total delta requirements.

### The Solution

Apply Step 1's expiry calculation independently for each path:

```
for each path in paths:
    hops = path.hops
    last_index = hops.len() - 1

    expires[last_index] = current_height + min_final_cltv

    for i from (last_index - 1) down to 0:
        expires[i] = expires[i + 1] + hops[i + 1].cltv_delta
```

### Key Insight

The final hop expiry (`current_height + min_final_cltv`) is the same for all paths because they're all paying the same invoice. The intermediate expiries differ based on each path's CLTV delta requirements.

---

## Section 5: Output Generation

### The Problem

Output must contain all paths, with correct path_id values, in the same order as input (ascending path_id, hops in route order within each path).

### The Solution

1. Process paths in ascending order of path_id (0, 1, 2, ...)
2. For each path, output hops in order (first hop to last hop)
3. Set the path_id field to the actual path_id (not hardcoded 0)

### Output Format

Same as Step 1, but with variable path_id:

```
path_id,channel_name,htlc_amount_msat,htlc_expiry,tlv
```

---

## Code Changes Required

### Changes to main.rs

1. **After reading hops**: Group them by `path_id` into a map or vector of vectors

2. **Calculate amount per path**: `let amount_per_path = invoice_amount / num_paths;`

3. **Loop over paths**: Process each path independently
   - Calculate amounts starting from `amount_per_path` (not `invoice_amount`)
   - Calculate expiries (same logic as before)

4. **Output**: Iterate through paths in order, writing all hops for each path

### Pseudocode for Main Logic

```rust
// Group hops by path_id
let mut paths: BTreeMap<u32, Vec<Hop>> = BTreeMap::new();
for hop in hops {
    paths.entry(hop.path_id).or_default().push(hop);
}

let num_paths = paths.len() as u64;
let amount_per_path = invoice_amount / num_paths;

// Process each path
for (path_id, path_hops) in paths {
    let last_index = path_hops.len() - 1;

    // Calculate amounts (backward)
    let mut amounts = vec![0u64; path_hops.len()];
    amounts[last_index] = amount_per_path;
    for i in (0..last_index).rev() {
        let fee = calculate_fee(&path_hops[i + 1], amounts[i + 1]);
        amounts[i] = amounts[i + 1] + fee;
    }

    // Calculate expiries (backward)
    let mut expires = vec![0u32; path_hops.len()];
    expires[last_index] = height + min_final_cltv;
    for i in (0..last_index).rev() {
        expires[i] = expires[i + 1] + path_hops[i + 1].cltv_delta;
    }

    // Write output rows for this path
    for i in 0..path_hops.len() {
        writer.serialize(OutputRow::new(
            path_id,
            path_hops[i].channel_name.clone(),
            amounts[i],
            expires[i],
            "NULL".to_string(),
        ));
    }
}
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

**Invoice**: 100,000 msat, min_final_cltv = 18
**Current height**: 800,000
**Number of paths**: 2
**Amount per path**: 50,000 msat

### Path 0 Calculations

**Amounts (backward):**
1. CarolDave: 50,000 msat (amount_per_path)
2. BobCarol: Carol's fee = 0 + (50,000 * 3000 / 1,000,000) = 150 msat
   - Amount = 50,000 + 150 = 50,150 msat
3. AliceBob: Bob's fee = 2000 + (50,150 * 500 / 1,000,000) = 2000 + 25 = 2,025 msat
   - Amount = 50,150 + 2,025 = 52,175 msat

**Expiries (backward):**
1. CarolDave: 800,000 + 18 = 800,018
2. BobCarol: 800,018 + 15 = 800,033
3. AliceBob: 800,033 + 65 = 800,098

### Path 1 Calculations

**Amounts (backward):**
1. EveDave: 50,000 msat (amount_per_path)
2. AliceEve: Eve's fee = 15 + (50,000 * 2500 / 1,000,000) = 15 + 125 = 140 msat
   - Amount = 50,000 + 140 = 50,140 msat

**Expiries (backward):**
1. EveDave: 800,000 + 18 = 800,018
2. AliceEve: 800,018 + 40 = 800,058

### Output

```
0,AliceBob,52175,800098,NULL
0,BobCarol,50150,800033,NULL
0,CarolDave,50000,800018,NULL
1,AliceEve,50140,800058,NULL
1,EveDave,50000,800018,NULL
```

---

## Backward Compatibility

The solution must still work for Step 1 inputs (single path with path_id = 0):
- When there's only one path, `num_paths = 1`
- `amount_per_path = invoice_amount / 1 = invoice_amount`
- Result is identical to Step 1

---

## Dependencies

No new dependencies needed. Use `BTreeMap` from std::collections for ordered path grouping (ensures paths are output in ascending path_id order).

---

## Verification

1. Test with Step 1 inputs to ensure backward compatibility
2. Test with multiple paths of varying lengths
3. Verify total amounts: sum of final hop amounts across all paths should equal invoice amount
4. Run `./test/check_run.sh`
