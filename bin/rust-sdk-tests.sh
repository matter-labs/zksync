#!/bin/bash

set -e
trap cat_logs EXIT

TIMEOUT_PID=""
PROVER_PID=""
SERVER_PID=""

function timeout() {
    sleep 1200
    echo Timeout is reached
    kill -s TERM "$1"
}

timeout "$$" &
TIMEOUT_PID=$!

function cat_logs() {
    exitcode=$?
    echo Termination started

    # Wait for server to finish any ongoing jobs
    sleep 5

    set +e
    pkill -P $SERVER_PID
    pkill -P $PROVER_PID
    pkill -P $TIMEOUT_PID
    echo Server logs:
    cat rust-sdk-server.log 
    echo ===========
    echo Prover logs:
    cat rust-sdk-prover.log

    # Wait for server to be surely killed
    sleep 5

    exit $exitcode
}

zksync dummy-prover status | grep -q 'disabled' && zksync dummy-prover enable

# We have to compile binaries, because otherwise the time to compile it may exceed 15 seconds,
# and the test will start without an actually running server.
f cargo build --bin zksync_server --release
f cargo build --bin dummy_prover --release

zksync server &> rust-sdk-server.log &
SERVER_PID=$!
# Wait a bit, so server and prover won't have conflicts about the workspace lockfile.
sleep 1 
zksync dummy-prover &> rust-sdk-prover.log &
PROVER_PID=$!

sleep 10
echo Performing rust SDK tests...
cargo test -p zksync --release -- --ignored --test-threads=1
