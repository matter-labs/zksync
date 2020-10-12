#!/bin/bash

set -e

trap cat_logs EXIT

SERVER_PID=""
PROVER_PID=""
TIMEOUT_PID=""

function timeout() {
  sleep 1200
  echo "Timeout is reached"
  kill -s TERM "$1"
}

timeout "$$" &
TIMEOUT_PID=$!

function cat_logs() {
    exitcode=$?
    echo "Termination started"
    # Wait for server to finish any ongoing jobs
    sleep 30

    set +e
    pkill -P $SERVER_PID
    pkill -P $PROVER_PID
    pkill -P $TIMEOUT_PID
    echo Server logs:
    cat integration-server.log
    echo ===========
    echo Prover logs:
    cat integration-prover.log

    # Wait for server to be surely killed
    sleep 10

    exit $exitcode
}

zksync dummy-prover status | grep -q 'disabled' && zksync dummy-prover enable

# We have to compile server, because otherwise the time to compile it may exceed 15 seconds,
# and the test will start without an actually running server.
f cargo build --bin zksync_server --release
zksync server &> integration-server.log &
SERVER_PID=$!
zksync dummy-prover &> integration-prover.log &
PROVER_PID=$!

sleep 15
zksync integration-test
zksync api-test
