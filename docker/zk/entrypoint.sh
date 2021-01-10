#!/bin/bash -ue

zk
# Prepare dummy-prover in the contract (so the redeployed version will be OK)
zk dummy-prover enable --no-redeploy

# Initialize the stack
zk init

echo "Done!"
