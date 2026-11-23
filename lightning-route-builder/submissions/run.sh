#!/bin/bash
# Check if four arguments are provided
if [ "$#" -ne 4 ]; then
    echo "Usage: $0 <output_file_path> <input_file_path> <payment_request_bech32> <current_block_height>"
    exit 1
fi

# Assign arguments to variables
output_file_path="$1"
input_file_path="$2"
payment_request_bech32="$3"
current_block_height="$4"

version="rustc 1.75.0"

# Build and run the Rust solution
cd ./submissions
cargo build --release
./target/release/solution "$output_file_path" "$input_file_path" "$payment_request_bech32" "$current_block_height"
