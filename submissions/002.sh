#!/bin/bash
# Fill in lncli commands to lookup the base fee of the node below broadcast for the channel id provided.

node="02a04400bc8aaeb00046deebb148eac438a91bcfc6307d8432eece0cdb35915961"
channel_id="362838837231616"

# This lncli command allows the script to run against signet nodes for testing purposes, refer to it using $lncli.
# You may comment it out and use your own testing node if you would like, but this line *must* be uncommented when you push to test.
wd=$(pwd)
lncli="lncli -n=signet --rpcserver=signet.bosschallenge.xyz:10009 --tlscertpath="$wd"/test/lnd0-tls.cert --macaroonpath="$wd"/test/lnd0-readonly.macaroon "

# Query the gossip database to find the channel and extract the base fee from the node's policy
$lncli describegraph | jq '.edges[] | select(.channel_id == "'"$channel_id"'") | if .node1_pub == "'"$node"'" then .node1_policy.fee_base_msat else .node2_policy.fee_base_msat end'
