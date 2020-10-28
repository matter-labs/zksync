#!/bin/bash -ue

# Run geth
nohup /usr/local/bin/geth-entry.sh &>/dev/null &

# Initialize database
service postgresql restart

# Prepare dummy-prover in the contract (so the redeployed version will be OK)
zksync dummy-prover enable-no-redeploy

# Initialize the stack (mostly, it's an init command with some steps skipped for docker environment)
zksync verify-keys unpack
zksync yarn || true # It can fail.
zksync db-setup
zksync build-dev-contracts
zksync deploy-erc20 dev
zksync build-contracts
zksync genesis
zksync redeploy

# Compile required dependencies
f cargo build --bin zksync_server --release
f cargo build --bin dummy_prover --release
f cargo build --bin dev-ticker-server --release

# Launch binaries
echo "Launching dev-ticker-server..."
nohup f $ZKSYNC_HOME/target/release/dev-ticker-server &>/dev/null &
sleep 1

echo "Launching server..."
nohup f $ZKSYNC_HOME/target/release/zksync_server &>$ZKSYNC_HOME/server.log &
sleep 1

echo "Launching dummy-prover..."
nohup f $ZKSYNC_HOME/target/release/dummy_prover "dummy-prover-instance" &>$ZKSYNC_HOME/dummy_prover.log &

# Wait for server to start
sleep 10

echo "Done!"
