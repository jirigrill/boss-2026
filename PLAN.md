# Plan: Lightning Payment Route Builder - Step 1 (Rust)

## BOLT7 Fee Calculation - Gist

The Lightning Network charges two types of fees for forwarding payments:

1. **Base fee** - A fixed amount in millisatoshis charged for every HTLC, regardless of size
2. **Proportional fee** - A variable amount based on the payment size, expressed in parts-per-million (ppm)

### The Fee Formula

```
fee = base_fee_msat + (amount_msat * proportional_fee_ppm / 1,000,000)
```

Use integer division (round down). In Rust, integer division is automatic.

**Example**: If base fee is 1000 msat, proportional fee is 500 ppm, and amount is 1,000,000 msat:
- Fee = 1000 + (1,000,000 * 500 / 1,000,000) = 1000 + 500 = 1500 msat

---

## Critical Insight: Calculate Backwards

When building a Lightning route, you must calculate **from destination back to source**. This is because:

- The receiver (final node) must get the exact invoice amount
- Each forwarding node takes a fee from what they receive before passing it on
- Therefore, the sender must send more than the invoice amount to cover all fees

Think of it like shipping costs: if you want the recipient to receive $100, and each handler takes a cut, you need to start with more money.

---

## Section 1: Project Setup

1. Create a new Rust binary project
2. Add the `csv` crate for reading and writing CSV files
3. Add a Lightning invoice parsing crate (such as `lightning-invoice`) to decode the bech32 payment request
4. Optionally add `serde` to make CSV parsing easier

Define two data structures:
- One for reading input rows (hop info with fees and deltas)
- One for writing output rows (HTLC amount, expiry, etc.)

---

## Section 2: Input Parsing

### Command Line Arguments

Your program receives four arguments:
1. Directory where to write `output.csv`
2. Path to input CSV file with route hops
3. Bech32-encoded payment request (invoice)
4. Current blockchain height as a number

### Reading the CSV

Read the input CSV file. Each row represents one hop in the payment route, containing:
- Channel name
- CLTV delta (time lock requirement)
- Base fee in millisatoshis
- Proportional fee in parts-per-million

### Parsing the Invoice

The payment request contains:
- The amount to pay (in millisatoshis)
- The minimum final CLTV expiry delta (how much time buffer the receiver needs)

The invoice uses bech32 encoding with prefix `lnbcrt` (regtest network).

---

## Section 3: Calculating HTLC Amounts

### The Problem

Given a route: `Alice -> Bob -> Carol -> Dave` (where Dave is the receiver)

The hops in the CSV are:
- AliceBob (first channel)
- BobCarol (middle channel)
- CarolDave (last channel)

Dave needs to receive the exact invoice amount. But Bob and Carol each charge fees for forwarding. So Alice must send more than the invoice amount.

### The Solution - Work Backwards

Start at the last hop and work towards the first:

**Step 1**: The final hop (CarolDave) carries exactly the invoice amount. Dave receives this directly.

**Step 2**: For the second-to-last hop (BobCarol), calculate what Carol charges to forward to Dave. Bob must send enough to cover both: the amount for Dave PLUS Carol's forwarding fee.

**Step 3**: Continue backwards. For each hop, the amount equals: the next hop's amount PLUS the fee that the next node charges for forwarding.

### Pseudocode

```
Start with amounts array of size = number of hops

amounts[last] = invoice_amount

For each hop from second-to-last down to first:
    next_hop_amount = amounts[hop + 1]
    forwarding_fee = calculate_fee(next_hop_policy, next_hop_amount)
    amounts[hop] = next_hop_amount + forwarding_fee
```

### Important Detail About Fees

The fee at hop `i` is calculated using the **next hop's** (hop `i+1`) fee policy. This is because the fee policy describes what a node charges for forwarding OUT of their channel, not what they charge for receiving.

---

## Section 4: Calculating CLTV Expiries

### The Problem

Each HTLC has a timeout (block height). If the payment isn't claimed by then, the funds return to the sender. Each forwarding node needs enough time buffer between their incoming and outgoing HTLCs.

### The Solution - Also Work Backwards

**Step 1**: The final hop's expiry is: `current_height + min_final_cltv_expiry` (from the invoice).

**Step 2**: For each preceding hop, add the CLTV delta of the next hop. This ensures each forwarder has enough time buffer.

### Pseudocode

```
expiries[last] = current_height + invoice_min_final_cltv

For each hop from second-to-last down to first:
    expiries[hop] = expiries[hop + 1] + next_hop_cltv_delta
```

### Important Detail About CLTV Deltas

Similar to fees, the CLTV delta at hop `i` comes from the **next hop's** (hop `i+1`) policy. The first hop's CLTV delta from the CSV is not used in calculation (it's the sender's advertised policy for others routing through them).

---

## Section 5: Writing the Output

Create a CSV file called `output.csv` in the specified output directory.

For each hop (in original order from first to last), write:
- path_id: 0 (hardcoded for Step 1)
- channel_name: from input
- htlc_amount_msat: calculated amount
- htlc_expiry: calculated expiry
- tlv: "NULL" (hardcoded for Step 1)

Do NOT include a header row in the output.

---

## Worked Example

**Input:**
```
path_id,channel_name,cltv_delta,base_fee_msat,proportional_fee_ppm
0,AliceBob,40,1000,10
0,BobCarol,65,2000,500
0,CarolDave,15,0,3000
```

**Invoice**: 100,000 msat, min_final_cltv = 18
**Current height**: 800,000

### Amounts (calculated backwards):

1. **CarolDave**: 100,000 msat (invoice amount, no calculation needed)

2. **BobCarol**: Carol charges 0 + (100,000 * 3000 / 1,000,000) = 300 msat to forward
   - Amount = 100,000 + 300 = 100,300 msat

3. **AliceBob**: Bob charges 2000 + (100,300 * 500 / 1,000,000) = 2000 + 50 = 2,050 msat to forward
   - Amount = 100,300 + 2,050 = 102,350 msat

### Expiries (calculated backwards):

1. **CarolDave**: 800,000 + 18 = 800,018
2. **BobCarol**: 800,018 + 15 (CarolDave's delta) = 800,033
3. **AliceBob**: 800,033 + 65 (BobCarol's delta) = 800,098

### Output:
```
0,AliceBob,102350,800098,NULL
0,BobCarol,100300,800033,NULL
0,CarolDave,100000,800018,NULL
```

---

## Verification

1. Manually verify with the worked example above
2. Run the provided test script: `./test/check_run.sh`
3. Test edge cases: single hop route, zero fees, large amounts
