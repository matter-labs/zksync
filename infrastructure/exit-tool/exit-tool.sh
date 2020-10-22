#!/bin/bash

USAGE='Usage: exit-tool.sh [-h|--help|init|run|continue]
where
    -h | --help       show this message
    init              prepare the database for data restore
    run               start the data restoring process
    continue          continue the interrupted data restoring process

If command is "run" or "continue", the following additional arguments are required:
    network           Ethereum network (rinkeby / ropsten / mainnet)
    account id        ID of account in zkSync network to generate exit proof for
    token             Token for which proof will be generated (may be numeric token ID, address or symbol, e.g. ETH)
    web3 url          Address of the HTTP Web3 API, which will be used to gather data from Ethereum.

Example workflow:

./exit-tool.sh init
./exit-tool.sh run rinkeby 12 ETH http://127.0.0.1:8545
'

COMMAND=$1

CURRENT_DIR=`pwd`
mkdir -p "$CURRENT_DIR/setup"
KEYS_FOLDER_LOCAL="$CURRENT_DIR/setup"
KEYS_FOLDER_CONTAINER="/usr/src/zksync/keys/setup"

case $COMMAND in
  init)
    SUBCOMMAND="init"
    ;;
  run | continue)
    NETWORK=$2
    ACCOUNT_ID=$3
    TOKEN=$4
    WEB3_URL=$5

    SUBCOMMAND="$COMMAND $NETWORK $ACCOUNT_ID $TOKEN $WEB3_URL"
    ;;
  -h | --help)
      echo "$USAGE"
      exit 0
    ;;
  *)
      echo "Unknown Data Restore command"
      echo "$USAGE"
      exit 1
    ;;
esac

docker-compose up -f ./docker-compose.yml -d postgres

DOCKER_ARGS="--net=host -v $KEYS_FOLDER_LOCAL:$KEYS_FOLDER_CONTAINER"
DOCKER_IMAGE="matterlabs/exit-tool:latest"
DOCKER_COMMAND="exit-tool-entry.sh $SUBCOMMAND"

docker run $DOCKER_ARGS $DOCKER_IMAGE $DOCKER_COMMAND
