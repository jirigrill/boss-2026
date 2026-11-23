# Coin Selection Solution Plan

## 1. Problem Understanding

### Challenge Goal
Process payments sequentially while minimizing total fees spent. The final score is the remaining wallet balance.

### Constraints
- Each transaction must meet the required fee rate (BTC/kvB)
- Fee rate = (total_input_value - total_output_value) / virtual_transaction_size
- Payments must be processed in order
- Each transaction must be confirmed before the next

### Key Insight
The fee rate varies across payments. The algorithm must make optimal decisions using only **current information**: the available UTXO pool and the current payment requirements.

---

## 2. Transaction Size Constants

### Input Sizes (vbytes)
```
P2PKH (legacy):           148 vB
P2SH-P2WPKH (nested):      91 vB
P2WPKH (native segwit):    68 vB
P2TR (taproot):          57.5 vB
```

### Output Sizes (vbytes)
```
P2PKH:    34 vB
P2SH:     32 vB
P2WPKH:   31 vB
P2TR:     43 vB
```

### Base Transaction Overhead
```
Legacy tx (no witness):  10 vB    (version + input_count + output_count + locktime)
SegWit tx:             10.5 vB    (adds 0.5 vB for marker + flag)
```

### Detecting UTXO Type from Address

```pseudo
function get_input_vbytes(address):
    if address starts with "bcrt1p" or "bc1p":
        return 57.5                     # P2TR
    else if address starts with "bcrt1q" or "bc1q":
        return 68                       # P2WPKH
    else if address starts with "2" or "3":
        return 91                       # P2SH-P2WPKH
    else:  # starts with "m", "n", or "1"
        return 148                      # P2PKH
```

### Detecting Output Type from Destination Address

```pseudo
function get_output_vbytes(address):
    if address starts with "bcrt1p" or "bc1p":
        return 43                       # P2TR output
    else if address starts with "bcrt1q" or "bc1q":
        return 31                       # P2WPKH output
    else if address starts with "2" or "3":
        return 32                       # P2SH output
    else:
        return 34                       # P2PKH output
```

---

## 3. Core Concepts

### 3.1 Effective Value

The "effective value" of a UTXO accounts for the cost to spend it at the current fee rate:

```pseudo
effective_value(utxo, fee_rate) = utxo.amount - (input_vbytes(utxo) * fee_rate)
```

A UTXO with negative effective value is **dust** at the current fee rate - it costs more to spend than it's worth.

### 3.2 Waste Metric

The waste metric combines:
1. **Input cost difference**: Cost of spending inputs at current rate vs. a "long-term" expected fee rate
2. **Change cost**: Cost of creating and later spending a change output

```pseudo
LONG_TERM_FEE_RATE = estimate based on typical network conditions (e.g., 10-20 sat/vB)

waste = input_waste + change_waste

input_waste = sum(input_vbytes) * (current_rate - LONG_TERM_FEE_RATE)
change_waste = (change_output_vbytes + change_spend_vbytes) * LONG_TERM_FEE_RATE
             = 0  # if no change output
```

**Key insight**:
- When `current_rate > LONG_TERM_RATE`: Prefer fewer inputs (minimize overpaying)
- When `current_rate < LONG_TERM_RATE`: Prefer more inputs (consolidate while cheap)
- Always prefer no change output when possible

### 3.3 Changeless Transactions

If you can find an input set where the excess is small enough to donate as fee:

```pseudo
excess = sum(inputs) - target_amount - base_fee
if excess < DUST_THRESHOLD:
    # No change needed - excess becomes additional fee
```

Benefits:
- Save change output overhead (~31-43 vB)
- Save future cost of spending that change (~57.5-148 vB)

---

## 4. Algorithm: Branch and Bound

Branch and Bound searches for an input set that creates a **changeless transaction** (or minimal waste).

### Branch and Bound Pseudocode

```pseudo
function branch_and_bound(utxos, target_amount, fee_rate, max_iterations = 100000):
    # Calculate effective values and filter out dust
    viable_utxos = []
    for utxo in utxos:
        eff_val = effective_value(utxo, fee_rate)
        if eff_val > 0:
            viable_utxos.append((utxo, eff_val))

    # Sort by effective value descending (for better pruning)
    sorted_utxos = sort(viable_utxos, by: eff_val, descending: true)

    # Target: amount + base tx fee (1 output, no change)
    base_tx_vbytes = 10.5 + get_output_vbytes(payment.address)  # overhead + 1 output
    # Note: input vbytes added dynamically as we select inputs

    best_selection = None
    best_waste = INFINITY

    # DFS with stack: (index, selected_indices, accumulated_effective_value)
    stack = [(0, [], 0)]
    iterations = 0

    while stack not empty and iterations < max_iterations:
        iterations += 1
        (idx, selected, acc_eff_value) = stack.pop()

        # Calculate actual values for current selection
        if len(selected) > 0:
            total_input_value = sum(sorted_utxos[i].amount for i in selected)
            total_input_vbytes = sum(sorted_utxos[i].input_vbytes for i in selected)
            tx_vbytes = base_tx_vbytes + total_input_vbytes
            required_fee = tx_vbytes * fee_rate
            excess = total_input_value - target_amount - required_fee

            # Check if this is a valid changeless solution
            if excess >= 0 and excess < DUST_THRESHOLD:
                waste = calculate_waste(selected, fee_rate, excess)
                if waste < best_waste:
                    best_waste = waste
                    best_selection = selected.copy()
                continue  # Found solution, don't add more inputs

            # Pruning: if we've exceeded target by too much, skip
            if excess > MAX_EXCESS_THRESHOLD:
                continue

        # Pruning: check if remaining UTXOs can possibly reach target
        remaining_eff_value = sum(eff_val for (_, eff_val) in sorted_utxos[idx:])
        if acc_eff_value + remaining_eff_value < target_amount:
            continue  # Can't possibly reach target

        if idx >= len(sorted_utxos):
            continue

        # Branch: try INCLUDING current UTXO
        utxo, eff_val = sorted_utxos[idx]
        stack.append((idx + 1, selected + [idx], acc_eff_value + eff_val))

        # Branch: try EXCLUDING current UTXO
        stack.append((idx + 1, selected.copy(), acc_eff_value))

    return best_selection  # May be None if no changeless solution found
```

### Waste Calculation

```pseudo
function calculate_waste(selected_indices, fee_rate, excess_to_fee):
    LONG_TERM_RATE = 15 sat/vB  # Configurable estimate

    total_input_vbytes = sum(input_vbytes for selected)

    # Input waste: difference between paying now vs at long-term rate
    input_waste = total_input_vbytes * (fee_rate - LONG_TERM_RATE)

    # Excess waste: overpaying fee beyond minimum required
    excess_waste = excess_to_fee

    # No change waste (changeless transaction)
    change_waste = 0

    return input_waste + excess_waste + change_waste
```

---

## 5. Fallback: Greedy Selection with Change

If Branch and Bound doesn't find a changeless solution, use greedy selection:

```pseudo
function select_coins_greedy(utxos, payment):
    fee_rate = payment.rate
    target = payment.amount

    # Filter dust and sort by effective value descending
    viable = [(u, effective_value(u, fee_rate)) for u in utxos if effective_value(u, fee_rate) > 0]
    sorted_utxos = sort(viable, by: eff_val, descending: true)

    selected = []
    total_value = 0

    for (utxo, _) in sorted_utxos:
        selected.append(utxo)
        total_value += utxo.amount

        # Calculate fee with change output
        tx_vbytes = calculate_tx_vbytes(selected, has_change: true, payment.address)
        required = target + (tx_vbytes * fee_rate)

        if total_value >= required:
            return selected

    # If we get here, insufficient funds (shouldn't happen per problem statement)
    return selected
```

---

## 6. Adaptive Strategy Based on Fee Rate

```pseudo
LONG_TERM_FEE_RATE = 15 sat/vB  # Tune based on expected conditions

function select_coins(utxos, payment):
    fee_rate = payment.rate

    # Step 1: Try Branch and Bound for changeless transaction
    bnb_result = branch_and_bound(utxos, payment.amount, fee_rate)
    if bnb_result is not None:
        return bnb_result

    # Step 2: Adaptive fallback based on fee environment
    if fee_rate < LONG_TERM_FEE_RATE:
        # LOW FEE: Opportunity to consolidate small UTXOs
        return select_for_consolidation(utxos, payment)
    else:
        # HIGH FEE: Minimize inputs to reduce costs
        return select_minimum_inputs(utxos, payment)
```

### Consolidation Strategy (Low Fees)

```pseudo
function select_for_consolidation(utxos, payment):
    fee_rate = payment.rate

    # Sort by amount ASCENDING (spend small UTXOs first)
    sorted_utxos = sort(utxos, by: amount, ascending: true)

    selected = []
    total_value = 0

    for utxo in sorted_utxos:
        # Skip dust
        if effective_value(utxo, fee_rate) <= 0:
            continue

        selected.append(utxo)
        total_value += utxo.amount

        tx_vbytes = calculate_tx_vbytes(selected, has_change: true, payment.address)
        required = payment.amount + (tx_vbytes * fee_rate)

        if total_value >= required:
            # We have enough - stop here
            break

    return selected
```

### Minimum Inputs Strategy (High Fees)

```pseudo
function select_minimum_inputs(utxos, payment):
    fee_rate = payment.rate

    # Sort by effective value DESCENDING (largest effective value first)
    viable = [(u, effective_value(u, fee_rate)) for u in utxos if effective_value(u, fee_rate) > 0]
    sorted_utxos = sort(viable, by: eff_val, descending: true)

    # Try single UTXO first
    for (utxo, _) in sorted_utxos:
        tx_vbytes = calculate_tx_vbytes([utxo], has_change: true, payment.address)
        required = payment.amount + (tx_vbytes * fee_rate)

        if utxo.amount >= required:
            return [utxo]

    # No single UTXO works - use greedy
    return select_coins_greedy(utxos, payment)
```

---

## 7. Transaction Size Calculation

```pseudo
function calculate_tx_vbytes(inputs, has_change, output_address):
    # Check if any input requires witness
    has_witness = any(is_segwit(input) for input in inputs)

    # Base overhead
    base = 10.5 if has_witness else 10.0

    # Sum input sizes
    input_vbytes = sum(get_input_vbytes(input.address) for input in inputs)

    # Payment output
    output_vbytes = get_output_vbytes(output_address)

    # Change output (if needed)
    if has_change:
        # Use efficient change type - P2WPKH is good balance of size and compatibility
        change_vbytes = 31  # P2WPKH

    total = base + input_vbytes + output_vbytes
    if has_change:
        total += change_vbytes

    return total

function is_segwit(utxo):
    addr = utxo.address
    return addr.starts_with("bcrt1") or addr.starts_with("bc1") or
           addr.starts_with("2") or addr.starts_with("3")
```

---

## 8. Main Payment Loop - MVP Version

Start with this simplified version to get a working solution quickly:

```pseudo
DUST_THRESHOLD = 546 satoshis  # Optional - can start without this check

function make_payments_mvp(payments):
    for payment in payments:
        # Get current UTXO pool
        utxos = parse_utxos(bcli("listunspent"))

        # Select coins using greedy algorithm
        selected = select_coins_greedy(utxos, payment)

        # Calculate change
        total_input = sum(u.amount for u in selected)
        tx_vbytes = calculate_tx_vbytes(selected, has_change: true, payment.address)
        fee = tx_vbytes * payment.rate
        change_amount = total_input - payment.amount - fee

        # Decide if we create change output
        if change_amount > DUST_THRESHOLD:
            # Create change output
            change_address = bcli("getrawchangeaddress bech32")  # P2WPKH
            outputs = {
                payment.address: payment.amount,
                change_address: change_amount
            }
        else:
            # No change - excess becomes additional fee
            # (either because change would be dust, or barely any excess)
            outputs = {payment.address: payment.amount}

        # Create and broadcast transaction
        inputs_json = format_inputs(selected)
        outputs_json = format_outputs(outputs)

        raw_tx = bcli("createrawtransaction {inputs_json} {outputs_json}")
        signed_result = bcli("signrawtransactionwithwallet {raw_tx}")
        signed_hex = parse_hex(signed_result)

        bcli("sendrawtransaction {signed_hex} 0")
        bcli("generatetoaddress 1 bcrt1pqqqqrldkrl")
```

**MVP Simplifications:**
- Uses only greedy selection (no Branch and Bound)
- Single change calculation (assumes change output initially)
- Simple dust check: `change > DUST_THRESHOLD`
- Can even skip dust check initially: `if change_amount > 0`

**Why start here:**
- ✓ Fewest moving parts
- ✓ Easy to debug
- ✓ Gets you to working end-to-end solution
- ✓ Will pass autograder
- ✓ Foundation for optimizations

---

## 8b. Main Payment Loop - Optimized Version

Once MVP works, add optimizations for better score:

```pseudo
# Economic dust threshold - outputs below this cost more to spend than they're worth
# Note: This challenge has dustrelayfee=0, so no network-enforced dust limit
# 546 is the Bitcoin Core standard, but you could use a lower value (e.g., 300 sats)
DUST_THRESHOLD = 546 satoshis  # Tunable - experiment with different values

function make_payments(payments):
    for payment in payments:
        # Get current UTXO pool
        utxos = parse_utxos(bcli("listunspent"))

        # Select coins
        selected = select_coins(utxos, payment)

        # Calculate if we need change
        total_input = sum(u.amount for u in selected)

        # First check: can we go changeless?
        tx_vbytes_no_change = calculate_tx_vbytes(selected, has_change: false, payment.address)
        fee_no_change = tx_vbytes_no_change * payment.rate
        excess_no_change = total_input - payment.amount - fee_no_change

        if excess_no_change >= 0 and excess_no_change < DUST_THRESHOLD:
            # Changeless transaction - excess goes to miners
            # This is economically optimal: saves change output overhead
            outputs = {payment.address: payment.amount}
        else:
            # Need change output
            tx_vbytes = calculate_tx_vbytes(selected, has_change: true, payment.address)
            fee = ceiling(tx_vbytes * payment.rate)  # Round up to ensure fee rate met
            change_amount = total_input - payment.amount - fee

            if change_amount < DUST_THRESHOLD:
                # Change would be economically unviable - donate to fee instead
                # (costs more to spend later than it's worth)
                outputs = {payment.address: payment.amount}
            else:
                change_address = bcli("getrawchangeaddress bech32")  # P2WPKH
                outputs = {
                    payment.address: payment.amount,
                    change_address: change_amount
                }

        # Create and broadcast transaction
        inputs_json = format_inputs(selected)
        outputs_json = format_outputs(outputs)

        raw_tx = bcli("createrawtransaction {inputs_json} {outputs_json}")
        signed_result = bcli("signrawtransactionwithwallet {raw_tx}")
        signed_hex = parse_hex(signed_result)

        bcli("sendrawtransaction {signed_hex} 0")
        bcli("generatetoaddress 1 bcrt1pqqqqrldkrl")
```

---

## 9. JSON Formatting for RPC

```pseudo
function format_inputs(utxos):
    # Returns: [{"txid":"abc...","vout":0},{"txid":"def...","vout":1}]
    items = []
    for utxo in utxos:
        items.append({"txid": utxo.txid, "vout": utxo.vout})
    return json_encode(items)

function format_outputs(outputs_map):
    # Returns: {"address1":0.001,"address2":0.002}
    # Note: amounts in BTC with sufficient decimal precision
    return json_encode(outputs_map)
```

---

## 10. Implementation Checklist

**Recommended approach:** Build MVP first (Phase 1), test it end-to-end, then add optimizations (Phase 2).

### Phase 1: MVP (Minimum Viable Product)

**Essential for working solution:**

1. **Data structures** ✓ MVP
   - [ ] Coin struct with txid, vout, address, amount
   - [ ] Payment struct (already provided)

2. **Size calculation functions** ✓ MVP
   - [ ] `get_input_vbytes(address)` - detect type and return vbytes
   - [ ] `get_output_vbytes(address)` - detect type and return vbytes
   - [ ] `calculate_tx_vbytes(inputs, has_change, output_addr)`
   - [ ] `effective_value(utxo, fee_rate)`

3. **Selection algorithms** ✓ MVP
   - [ ] `select_coins_greedy()` - basic greedy selection only

4. **Transaction building** ✓ MVP
   - [ ] JSON formatting for inputs array
   - [ ] JSON formatting for outputs object
   - [ ] Fee calculation and change handling
   - [ ] Dust threshold checking (optional but recommended)

5. **RPC integration** ✓ MVP
   - [ ] Parse `listunspent` response
   - [ ] Handle `getrawchangeaddress` (specify bech32)
   - [ ] Extract hex from `signrawtransactionwithwallet` response

### Phase 2: Optimizations (Better Score)

**Add after MVP works:**

6. **Advanced selection algorithms** ⚡ Optimization
   - [ ] `branch_and_bound()` - find changeless transactions
   - [ ] `select_for_consolidation()` - smallest-first for low fees
   - [ ] `select_minimum_inputs()` - largest-first for high fees
   - [ ] `select_coins()` - main dispatcher with adaptive strategy

7. **Enhanced change logic** ⚡ Optimization
   - [ ] Changeless transaction detection (before calculating with change)
   - [ ] Waste metric calculation
   - [ ] Parameter tuning (dust threshold, long-term fee rate)

---

## 11. Tuning Parameters

```pseudo
# Dust threshold - change below this is donated to fee
# IMPORTANT: This challenge has dustrelayfee=0 in bitcoin.conf
# This means there's NO network-enforced dust limit for this regtest
# However, we still use an economic threshold to avoid creating
# change outputs that cost more to spend later than they're worth
#
# Options to consider:
# - 546 sats: Bitcoin Core standard (conservative)
# - 300 sats: More aggressive (still economically sound at moderate fees)
# - Experiment to find optimal balance for this challenge
DUST_THRESHOLD = 546 satoshis (0.00000546 BTC)  # Tunable parameter

# Long-term fee rate estimate for waste calculation
# Lower = more aggressive consolidation during low fees
# Higher = more conservative, fewer inputs overall
LONG_TERM_FEE_RATE = 10-20 sat/vB

# Branch and Bound iteration limit
# Higher = better solutions but slower
# Lower = faster but may miss optimal changeless
BNB_MAX_ITERATIONS = 100000

# Maximum excess to consider for changeless (beyond dust)
# Helps BnB pruning - don't explore solutions with too much excess
MAX_EXCESS_FOR_CHANGELESS = 1000 satoshis
```

---

## 12. Summary

### MVP Strategy (Start Here)

1. **Use greedy selection** - sort by effective value, pick largest first
2. **Simple change logic** - if change > DUST_THRESHOLD, create change output
3. **Donate tiny amounts to miners** - saves creating uneconomical outputs
4. **Use P2WPKH change addresses** - `getrawchangeaddress bech32`

**This will work and pass autograder.** Build this first, then optimize.

### Optimal Strategy (After MVP Works)

1. **Always try Branch and Bound first** - changeless transactions save the most
2. **At low fees**: consolidate small UTXOs (smallest-first selection)
3. **At high fees**: minimize inputs (largest effective value first)
4. **Never create dust change** - donate to fee instead
5. **Use efficient change addresses** - P2WPKH (bech32) for good balance

The waste metric guides decisions: minimize `(input_vbytes * fee_delta) + change_cost + excess_fee`.

---

## References

- [BIP 141 - Segregated Witness](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki)
- [Bitcoin Optech Transaction Size Calculator](https://bitcoinops.org/en/tools/calc-size/)
- [Weight Units - Bitcoin Wiki](https://en.bitcoin.it/wiki/Weight_units)
