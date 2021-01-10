#!/bin/bash -ue

# Run geth
nohup /usr/local/bin/geth-entry.sh &>/dev/null &

# Prepare dummy-prover in the contract (so the redeployed version will be OK)
zk dummy-prover enable --no-redeploy

# Initialize the stack
zk init

echo "Done!"
