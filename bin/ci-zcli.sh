#!/bin/bash

set -e
trap cat_logs EXIT
TIMEOUT_PID=""
SERVER_PID=""

function timeout() {
    sleep 300
    echo Timeout is reached
    kill -s TERM "$1"
}

function cat_logs() {
    exitcode=$?
    echo Termination started

    # Wait for server to finish any ongoing jobs
    sleep 5

    set +e
    pkill -P $TIMEOUT_PID,$SERVER_PID
    echo Server logs:
    cat zcli-test-server.log

    # Wait for server to be surely killed
    sleep 5

    exit $exitcode
}

timeout "$$" &
TIMEOUT_PID=$!

zksync server &> zcli-test-server.log &
SERVER_PID=$!

sleep 10
echo Performing zcli tests...
yarn --cwd infrastructure/zcli test
