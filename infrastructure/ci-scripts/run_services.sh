echo "Launching server..."
zk server &>server.log &
sleep 1

echo "Launching dummy-prover..."
zk dummy-prover run &>dummy_prover.log &

# Wait for server to start
sleep 10

echo "Done!"