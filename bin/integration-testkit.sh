#!/bin/bash

USAGE='Usage: zksync integration-testkit [ fast | block-sizes-test ]

Used to run testkit tests with geth configured for fast block execution.

Options:

fast (default) | block-sizes-test     select tests to run
'
COMMAND=${1:-fast}

. .setup_env

set -e

trap clean_up EXIT

PREV_WEB3_URL=$WEB3_URL

function clean_up() {
    exitcode=$?
    if [[ $ZKSYNC_ENV == dev ]]; then
        docker kill $CONTAINER_ID > /dev/null;
        if [[ $? != 0 && $CONTAINER_ID != '' ]]; then
            echo "problem killing $CONTAINER_ID"
        fi
    fi
    export WEB3_URL=$PREV_WEB3_URL
    exit $exitcode
}

# set up fast geth
if [[ $ZKSYNC_ENV == ci ]]; then
    export WEB3_URL=http://geth-fast:8545
elif [[ $ZKSYNC_ENV == dev ]]; then
    CONTAINER_ID=$(docker run --rm -d -p 7545:8545 matterlabs/geth:latest fast)
    export WEB3_URL=http://localhost:7545
fi

export ETH_NETWORK="test"
make build-contracts

case $COMMAND in
    block-sizes-test)
        cargo run --bin block_sizes_test --release
        ;;
    fast)
        cargo run --bin zksync_testkit --release
        cargo run --bin gas_price_test --release
        cargo run --bin migration_test --release
        cargo run --bin revert_blocks_test --release
        cargo run --bin exodus_test --release
        ;;
    -h|--h)
        echo "$USAGE" && exit 0
        ;;
    *)
        echo "$USAGE" && exit 1
        ;;
esac
