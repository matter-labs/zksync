#!/bin/bash

# Usage: governance-add-erc20.sh erc20_token_address
# Adds new ERC20 token to our network
. .setup_env

.confirm_action || exit 1

cd $ZKSYNC_HOME/contracts

if [ $1 == "test"]; then 
  export ETH_NETWORK="test"

if [ -n "$3" ]; then
  npx ts-node scripts/add-erc20-token.ts --tokenAddress $2 --deployerPrivateKey $3
else
  npx ts-node scripts/add-erc20-token.ts --tokenAddress $2
fi
