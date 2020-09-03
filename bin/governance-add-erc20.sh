#!/bin/bash

# Usage: governance-add-erc20.sh erc20_token_address
# Adds new ERC20 token to our network
. .setup_env

.confirm_action || exit 1

cd $ZKSYNC_HOME/contracts

if [ -n "$2" ]; then
  npx ts-node scripts/add-erc20-token.ts --tokenAddress $1 --deployerPrivateKey $2
else
  npx ts-node scripts/add-erc20-token.ts --tokenAddress $1
fi
