#!/bin/bash

# Set the `DUMMY_VERIFIER` constant value in the contract to `false`.
ssed -E "s/(.*constant DUMMY_VERIFIER)(.*)\;/\1 = false\;/" -i $ZKSYNC_HOME/contracts/contracts/Verifier.sol

# Reset the database and redeploy contracts.
zksync build-contracts   
zksync db-reset
zksync genesis
zksync redeploy
