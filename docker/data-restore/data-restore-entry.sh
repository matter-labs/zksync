#!/bin/bash

set -e

function reset_db() {
      cd core/lib/storage
      psql "$DATABASE_URL" -c 'DROP OWNED BY CURRENT_USER CASCADE' || /bin/true
      psql "$DATABASE_URL" -c 'DROP SCHEMA IF EXISTS public CASCADE' || /bin/true
      psql "$DATABASE_URL" -c 'CREATE SCHEMA public' || /bin/true
      diesel database setup
      cd $ZKSYNC_HOME
}

function migrate() {
      cd core/lib/storage
      diesel migration run
      cd $ZKSYNC_HOME
}

cd $ZKSYNC_HOME

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
      reset_db
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
  [ -f $PG_DUMP_PATH/$PG_DUMP ] || { echo "$PG_DUMP_PATH/$PG_DUMP  not found" ; exit 1 ; }

  reset_db

  echo "Applying $PG_DUMP"
  pg_restore -xO -j 8 -d $DATABASE_URL --clean --if-exists $PG_DUMP_PATH/$PG_DUMP
fi

if [[ -z $CONFIG_PATH ]]
then
  CONFIG_FILE="${ZKSYNC_HOME}/docker/exit-tool/configs/${NETWORK}.json"
else
  CONFIG_FILE="${CONFIG_PATH}/${NETWORK}.json"
fi

migrate

$ZKSYNC_HOME/target/release/zksync_data_restore $COMMAND $MODE --config $CONFIG_FILE --web3 $WEB3_URL || exit 1
