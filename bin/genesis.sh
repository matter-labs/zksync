#!/bin/bash

. .setup_env

set -e

cargo run --bin server -- --genesis | tee genesis.log

GENESIS_ROOT_NEW_VALUE=`grep GENESIS_ROOT genesis.log`

export LABEL=$FRANKLIN_ENV-Genesis_gen-`date +%Y-%m-%d-%H%M%S`
mkdir -p logs/$LABEL/
cp ./$ENV_FILE logs/$LABEL/$FRANKLIN_ENV.bak
cp genesis.log logs/$LABEL/
echo $GENESIS_ROOT_NEW_VALUE
python3 bin/replace-env-variable.py ./$ENV_FILE $GENESIS_ROOT_NEW_VALUE
