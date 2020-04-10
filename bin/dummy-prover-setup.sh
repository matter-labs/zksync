#!/bin/bash

# Set the `DUMMY_VERIFIER` constant value in the contract to `true`.
ssed -E "s/(.*constant DUMMY_VERIFIER)(.*)\;/\1 = true\;/" -i $ZKSYNC_HOME/contracts/contracts/Verifier.sol

echo "Enabled the Dummy Prover in the contract..."

# Reset the database and redeploy contracts.
zksync build-contracts   
zksync db-reset
zksync genesis
zksync redeploy

echo "All done"
