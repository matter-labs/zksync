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