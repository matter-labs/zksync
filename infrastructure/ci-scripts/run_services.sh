echo "Launching server..."
nohup zk server &>$ZKSYNC_HOME/server.log &
sleep 1

echo "Launching dummy-prover..."
nohup zk dummy_prover run &>$ZKSYNC_HOME/dummy_prover.log &

# Wait for server to start
sleep 10

echo "Done!"