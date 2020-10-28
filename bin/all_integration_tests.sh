#!/bin/bash
cd $ZKSYNC_HOME
. .setup_env

zksync integration-test || exit 1
zksync api-test || exit 1

# zcli test
yarn --cwd infrastructure/zcli test || exit 1

# rust-sdk test
cargo test -p zksync --release -- --ignored --test-threads=1 || exit 1

# We have to kill the server before running data-restore
killall zksync_server
zksync data-restore check-existing || exit 1
