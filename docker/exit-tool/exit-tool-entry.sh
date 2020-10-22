#!/bin/bash

# 1. Check whether database `plasma` exists
# 2. If not, run all the migrations
# 3. Run data-restore in the finite mode
# 4. Run gen-exit-proof

USAGE="exit_tool_entry.sh init|restart|run|continue account_id token web3_url"

. .setup_env

cd $ZKSYNC_HOME

if [ -z $ZKSYNC_ENV ];
then 
  echo "$USAGE"
  exit 1
fi

zksync plonk-setup check || zksync plonk-setup download

COMMAND=$1

case $COMMAND in
  init)
    f db-setup
    echo "Database set up"
    exit 0
    ;;
  run)
      f ./target/release/zksync_data_restore --genesis --finite --config_path=/usr/src/configs/rinkeby.json
    ;;
  continue)
      f ./target/release/zksync_data_restore --continue --finite --config_pathg=/usr/src/configs/rinkeby.json
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

