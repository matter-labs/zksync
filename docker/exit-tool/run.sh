#!/bin/bash

ACCOUNT_ID = $1
TOKEN = $2
WEB3_ADDR = $3

# TODO these values must be chosen for the expected network (mainnet / rinkeby / ropsten)
GENESIS_TX_HASH="0x0dc9b2387b0d3f80adcece49936ed3efec27c2845ed7ca72d01e567c872cfa1f"

docker run --net=host --mount type=bind,src=./setup,dst=/usr/src/zksync/keys/setup matterlabs/exit-tool:latest bash -c "exit-tool-entry.sh init"
docker run --net=host --mount type=bind,src=./setup,dst=/usr/src/zksync/keys/setup matterlabs/exit-tool:latest bash -c "exit-tool-entry.sh run $ACCOUNT_ID $TOKEN $WEB3_ADDR $GENESIS_TX_HASH"
