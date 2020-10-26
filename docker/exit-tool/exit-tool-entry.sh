#!/bin/bash

# 1. Check whether database `plasma` exists
# 2. If not, run all the migrations
# 3. Run data-restore in the finite mode
# 4. Run gen-exit-proof

USAGE="exit_tool_entry.sh init|restart|run|continue network account_id token web3_url"

. .setup_env

cd $ZKSYNC_HOME

if [ -z $ZKSYNC_ENV ];
then 
  echo "$USAGE"
  exit 1
fi

zksync plonk-setup check || zksync plonk-setup download
zksync verify-keys unpack
f db-wait

COMMAND=$1

case $COMMAND in
  init)
    f db-setup
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
ACCOUNT_ID=$3
TOKEN=$4
WEB3_URL=$5

CONFIG_FILE="/usr/src/configs/${NETWORK}.json"

# Set the required verification keys dir
case $NETWORK in
  mainnet | rinkeby | ropsten)
    export KEY_DIR=keys/plonk-975ae851
    ;;
  *)
      echo "Unknown Ethereum network"
      echo "$USAGE"
      exit 1
    ;;
esac

f ./target/release/zksync_data_restore $COMMAND --finite --config $CONFIG_FILE --web3 $WEB3_URL || exit 1

./target/release/examples/generate_exit_proof --account_id $ACCOUNT_ID --token $TOKEN
