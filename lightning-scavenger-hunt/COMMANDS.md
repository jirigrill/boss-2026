# Task 3: Custom Route Payment Commands

## Your Invoice Details
- **Wallet**: wallet_151
- **Amount**: 1984 sats
- **Payment Hash**: `34f43ad596cc655c6743c18f85f22a4393984d53ef7c6dbcb0ad32b283d54a78`
- **Payment Address**: `5d140f1dd2c3ae82f0f75a382bf5240fbfe564a158ac3565dd56053f1fd0314c`

## Route
```
Your Node --[chan 1903254627745792]--> lnd1 --[chan 362838837231616]--> lnd0
```

## Fee Structure
- lnd1 charges: base_fee=15000msat + fee_rate=20000ppm (2%)
- Fee for 1984 sats: 15000 + (1984000 * 20000 / 1000000) = 54680 msat
- Using 70000 msat (70 sats) to be safe with rounding
- **Important**: Fee goes on hop 2 (lnd1's fee), NOT hop 1 (your node doesn't charge itself)

## Option 1: Positional Argument (Recommended - Try First)

```bash
lncli sendtoroute --payment_hash=34f43ad596cc655c6743c18f85f22a4393984d53ef7c6dbcb0ad32b283d54a78 '{"route":{"total_time_lock":1884,"total_fees":"1000","total_amt":"2984","total_fees_msat":"1000000","total_amt_msat":"2984000","first_hop_amount_msat":"2984000","hops":[{"chan_id":"1903254627745792","chan_capacity":"50000","amt_to_forward":"2984","fee":"0","expiry":1884,"amt_to_forward_msat":"2984000","fee_msat":"0","pub_key":"02a04400bc8aaeb00046deebb148eac438a91bcfc6307d8432eece0cdb35915961","tlv_payload":true},{"chan_id":"362838837231616","chan_capacity":"100000000","amt_to_forward":"1984","fee":"1000","expiry":1824,"amt_to_forward_msat":"1984000","fee_msat":"1000000","pub_key":"02dbe21ebbd6253b2b0ba1f30cba7331e6e6d48aa289f15bf6d2f267b6c6480fa1","tlv_payload":true,"mpp_record":{"payment_addr":"5d140f1dd2c3ae82f0f75a382bf5240fbfe564a158ac3565dd56053f1fd0314c","total_amt_msat":"1984000"}}]}}'
```

## Option 2: Pipe via stdin

```bash
echo '{"route":{"total_time_lock":1884,"total_fees":"70","total_amt":"2054","total_fees_msat":"70000","total_amt_msat":"2054000","first_hop_amount_msat":"2054000","hops":[{"chan_id":"1903254627745792","chan_capacity":"50000","amt_to_forward":"2054","fee":"0","expiry":1884,"amt_to_forward_msat":"2054000","fee_msat":"0","pub_key":"02a04400bc8aaeb00046deebb148eac438a91bcfc6307d8432eece0cdb35915961","tlv_payload":true},{"chan_id":"362838837231616","chan_capacity":"100000000","amt_to_forward":"1984","fee":"70","expiry":1824,"amt_to_forward_msat":"1984000","fee_msat":"70000","pub_key":"02dbe21ebbd6253b2b0ba1f30cba7331e6e6d48aa289f15bf6d2f267b6c6480fa1","tlv_payload":true,"mpp_record":{"payment_addr":"5d140f1dd2c3ae82f0f75a382bf5240fbfe564a158ac3565dd56053f1fd0314c","total_amt_msat":"1984000"}}]}}' | lncli sendtoroute --payment_hash=34f43ad596cc655c6743c18f85f22a4393984d53ef7c6dbcb0ad32b283d54a78 -
```

## Option 3: Here-doc (More Readable)

```bash
lncli sendtoroute --payment_hash=34f43ad596cc655c6743c18f85f22a4393984d53ef7c6dbcb0ad32b283d54a78 - << 'EOF'
{
  "route": {
    "total_time_lock": 1884,
    "total_fees": "70",
    "total_amt": "2054",
    "total_fees_msat": "70000",
    "total_amt_msat": "2054000",
    "first_hop_amount_msat": "2054000",
    "hops": [
      {
        "chan_id": "1903254627745792",
        "chan_capacity": "50000",
        "amt_to_forward": "2054",
        "fee": "0",
        "expiry": 1884,
        "amt_to_forward_msat": "2054000",
        "fee_msat": "0",
        "pub_key": "02a04400bc8aaeb00046deebb148eac438a91bcfc6307d8432eece0cdb35915961",
        "tlv_payload": true
      },
      {
        "chan_id": "362838837231616",
        "chan_capacity": "100000000",
        "amt_to_forward": "1984",
        "fee": "70",
        "expiry": 1824,
        "amt_to_forward_msat": "1984000",
        "fee_msat": "70000",
        "pub_key": "02dbe21ebbd6253b2b0ba1f30cba7331e6e6d48aa289f15bf6d2f267b6c6480fa1",
        "tlv_payload": true,
        "mpp_record": {
          "payment_addr": "5d140f1dd2c3ae82f0f75a382bf5240fbfe564a158ac3565dd56053f1fd0314c",
          "total_amt_msat": "1984000"
        }
      }
    ]
  }
}
EOF
```

## Troubleshooting

### If you get "expiry too soon" error:
Get current block height and recalculate:
```bash
lncli getinfo | jq '.block_height'
```
Then update:
- `expiry` for hop 2 (lnd0): `block_height + 80`
- `expiry` for hop 1 (lnd1): `hop2_expiry + 60`
- `total_time_lock`: same as hop 1 expiry

### If payment succeeds:
The response will contain `payment_preimage` - save this to `submissions/003-preimage.txt`
