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

zksync dummy-prover enable
zksync server &> integration-server.log &
SERVER_PID=$!
zksync dummy-prover &> integration-prover.log &
PROVER_PID=$!

sleep 15
echo "Performing integration-simple test..."
zksync integration-simple
echo "Performing integration-simple test http provider..."
zksync integration-simple http
echo "Performing integration-full-exit test..."
zksync integration-full-exit
echo "Performing api-type-validate test..."
zksync api-type-validate
