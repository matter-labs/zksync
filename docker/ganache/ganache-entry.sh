#!/bin/sh

node ./generate-blocks.js "$$" & pid="$!"

trap ctrl_c INT TERM EXIT

function ctrl_c() {
    echo killing $pid
    kill $pid
    exit 0
}

exec yarn ganache-cli --accounts 100 --defaultBalanceEther 1000000 --mnemonic "$MNEMONIC" --port 7545 --host "0.0.0.0" 2>&1
