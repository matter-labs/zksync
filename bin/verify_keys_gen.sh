#!/bin/bash

set -e

. .setup_env

# HACK: We need to rebuild sources in case hardcoded values changed that might affect the keys
touch core/models/src/lib.rs

OUTPUT_DIR=$ZKSYNC_HOME/$KEY_DIR/account-"$ACCOUNT_TREE_DEPTH"_balance-"$BALANCE_TREE_DEPTH"/

mkdir -p $OUTPUT_DIR

cargo run --bin key_generator --release -- keys
cargo run --bin key_generator --release -- contract

cp $OUTPUT_DIR/Verifier.sol $ZKSYNC_HOME/contracts/contracts
