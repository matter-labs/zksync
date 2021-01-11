echo "Launching server..."
nohup zk f $ZKSYNC_HOME/target/release/zksync_server &>$ZKSYNC_HOME/server.log &
sleep 1

echo "Launching dummy-prover..."
nohup zk f $ZKSYNC_HOME/target/release/dummy_prover "dummy-prover-instance" &>$ZKSYNC_HOME/dummy_prover.log &

# Wait for server to start
sleep 10

echo "Done!"