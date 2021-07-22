#!/bin/bash

# 1. Check whether database `plasma` exists
# 2. If not, run all the migrations
# 3. Run data-restore in the finite mode
# 4. Run gen-exit-proof

USAGE="exit_tool_entry.sh init|restart|run|continue network account_id token web3_url"

cd $ZKSYNC_HOME

# if [ -z $ZKSYNC_ENV ];
# then 
#   echo "$USAGE"
#   exit 1
# fi

zk
zk run plonk-setup
zk run verify-keys unpack
zk db wait

COMMAND=$1

case $COMMAND in
  init)
    zk db basic-setup
    echo "Database set up"
    exit 0
    ;;
  run)
      COMMAND="--genesis"
    ;;
  continue)
      COMMAND="--continue"
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

# Load the rest of arguments now, since they're not required for the init command.
NETWORK=$2
ADDRESS=$3
TOKEN=$4
WEB3_URL=$5

CONFIG_FILE="/usr/src/configs/${NETWORK}.json"

# Set the required verification keys dir
case $NETWORK in
  mainnet | rinkeby | ropsten)
    export KEY_DIR=keys/contracts-4
    ;;
  *)
      echo "Unknown Ethereum network"
      echo "$USAGE"
      exit 1
    ;;
esac

zk f ./target/release/zksync_data_restore $COMMAND --finite --config $CONFIG_FILE --web3 $WEB3_URL || exit 1

zk f ./target/release/examples/generate_exit_proof --address $ADDRESS --token $TOKEN
