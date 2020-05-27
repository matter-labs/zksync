#!/bin/bash

set -e

trap cat_logs EXIT

SERVER_PID=""
PROVER_PID=""

function cat_logs() {
    exitcode=$?
    set +e
    kill -9 $SERVER_PID
    kill -9 $PROVER_PID
    echo Server logs:
    cat integration-server.log
    echo ===========
    echo Prover logs:
    cat integration-prover.log
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
echo "Performing integration-full-exit test..."
zksync integration-full-exit
echo "Performing api-type-validate test..."
zksync api-type-validate
