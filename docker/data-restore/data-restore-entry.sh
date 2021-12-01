#!/bin/bash

set -e

cd $ZKSYNC_HOME

# Load the environment
export $(cat $ZKSYNC_HOME/etc/env/docker.env | sed 's/#.*//g' | xargs)

# Wait for the database to be ready.
until pg_isready -d $DATABASE_URL; do
  sleep 1
done

if [[ -z $COMMAND || -z $NETWORK || -z $WEB3_URL ]]
then
  echo "Couldn't start the data restore, environment variables are missing"
  exit 1
fi

case $COMMAND in
  genesis)
      echo "Resetting the database"
      zk db drop || true
      zk db basic-setup
      COMMAND="--genesis"
    ;;
  continue)
      COMMAND="--continue"
    ;;
  *)
      echo "Unknown Data Restore command"
      exit 1
    ;;
esac

case $NETWORK in
  mainnet | rinkeby | ropsten)
    ;;
  *)
      echo "Unknown Ethereum network"
      exit 1
    ;;
esac

# Default to executing data restore in a finite mode.
MODE="--finite"
case $FINITE_MODE in
  "true" | "")
    ;;
  "false")
    MODE=""
    ;;
  *)
    echo "Invalid value of FINITE_MODE: expected boolean"
    exit 1
  ;;
esac

if [[ -n $PG_DUMP && "$COMMAND" == "--continue" ]]
then
  # Do not drop db if the file doesn't exist.
  [ -f /pg_restore/$PG_DUMP ] || { echo "$PG_DUMP not found" ; exit 1 ; }

  zk db drop || true
  zk db basic-setup
  echo "Applying $PG_DUMP"
  pg_restore -j 8 -d $DATABASE_URL --clean --if-exists /pg_restore/$PG_DUMP
fi

CONFIG_FILE="/usr/src/configs/${NETWORK}.json"

zk f ./target/release/zksync_data_restore $COMMAND $MODE --config $CONFIG_FILE --web3 $WEB3_URL || exit 1
