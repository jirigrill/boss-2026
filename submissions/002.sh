#!/bin/bash
# Fill in lncli commands to lookup the base fee of the node below broadcast for the channel id provided.

node="02aa3cd72329c8e0168998647c537a81266d65e37b70a9ae482dc5fe7d9c95d07f"
channel_id="356241767464960"

# This lncli command allows the script to run against signet nodes for testing purposes, refer to it using $lncli.
# You may comment it out and use your own testing node if you would like, but this line *must* be uncommented when you push to test.
wd=$(pwd)
lncli="lncli -n=signet --rpcserver=signet.bosschallenge.xyz:10009 --tlscertpath="$wd"/test/lnd0-tls.cert --macaroonpath="$wd"/test/lnd0-readonly.macaroon "
