#!/bin/bash -ue

# Run geth
nohup /usr/local/bin/geth-entry.sh &>/dev/null &

# Initialize database
service postgresql restart

# Prepare dummy-prover in the contract (so the redeployed version will be OK)
zk dummy-prover enable --no-redeploy

# Initialize the stack
zk run verify-keys unpack
zk run yarn || true # It can fail
zk db setup
zk contract build
zk run deploy-erc20 dev
zk run governance-add-erc20 dev
zk server --genesis
zk contract redeploy

# Compile required dependencies
zk f cargo build --bin zksync_server --release
zk f cargo build --bin dummy_prover --release
zk f cargo build --bin dev-ticker-server --release
zk f cargo build --bin dev-liquidity-token-watcher --release

# Launch binaries
echo "Launching dev-ticker-server..."
nohup zk f $ZKSYNC_HOME/target/release/dev-ticker-server &>/dev/null &
sleep 1

# Launch binaries
echo "Launching dev-liquidity-token-watcher..."
nohup zk f $ZKSYNC_HOME/target/release/dev-liquidity-token-watcher &>/dev/null &
sleep 1

echo "Launching server..."
nohup zk f $ZKSYNC_HOME/target/release/zksync_server &>$ZKSYNC_HOME/server.log &
sleep 1

echo "Launching dummy-prover..."
nohup zk f $ZKSYNC_HOME/target/release/dummy_prover "dummy-prover-instance" &>$ZKSYNC_HOME/dummy_prover.log &

# Wait for server to start
sleep 10

echo "Done!"
