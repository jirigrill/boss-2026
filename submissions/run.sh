#!/bin/bash
# Check if three arguments are provided
if [ "$#" -ne 4 ]; then
    echo "Usage: $0 <output_file_path> <input_file_path> <payment_request_bech32> <current_block_height>"
    exit 1
fi

# Assign arguments to variables
output_file_path="$1"
input_file_path="$2"
payment_request_bech32="$3"
current_block_height="$4"

# Please fill in the version of the programming language you used here to help us with debugging if we run into problems!
# version="{PLEASE FILL YOUR VERSION HERE}"

# Check if the 'version' variable is not null
if [ -z "$version" ]; then
    echo "Please fill in the version of the programming language you used."
    exit 1
fi

# Your run command here:
# (example)
# apt install -y python3 python3-pip
# python3 -m pip install {{... whatever libraries you need ...}}
# python3 ./submissions/{{... path to your code ...}} $output_file_path $input_file_path $payment_request_bech32 $current_block_height
