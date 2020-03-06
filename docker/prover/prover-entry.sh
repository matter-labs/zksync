#!/bin/sh

# remove quotes for docker-compose
# export KEY_FILES=`echo $KEY_FILES | sed -e 's/"\(.*\)/\1/g' -e 's/"$//g'`

echo NODE_NAME=$NODE_NAME
echo POD_NAME=$POD_NAME

. /bin/load_keys

echo key download complete, starting prover

exec prover $BLOCK_SIZE_CHUNKS "$POD_NAME" 2>&1
