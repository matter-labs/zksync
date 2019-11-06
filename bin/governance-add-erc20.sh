#!/bin/bash

# Usage: governance-add-erc20.sh erc20_token_address
# Adds new ERC20 token to our network
. .setup_env

.confirm_action || exit 1

cd $FRANKLIN_HOME/contracts
f npx ts-node scripts/add-erc20-token.ts $1
