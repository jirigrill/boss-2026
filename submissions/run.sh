#!/bin/bash
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 <output_directory> <input_file>"
    exit 1
fi

# Assign arguments to variables
output_directory="$1"
input_file="$2"

# Please fill in the version of the programming language you used here to help us with debugging if we run into problems!
# version="{PLEASE FILL YOUR VERSION HERE}"

# Check if the 'version' variable is not null
if [ -z "$version" ]; then
    echo "Please fill in the version of the programming language you used."
    exit 1
fi

# Your run command here:
