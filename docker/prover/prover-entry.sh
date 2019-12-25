#!/bin/sh

# remove quotes for docker-compose
# export KEY_FILES=`echo $KEY_FILES | sed -e 's/"\(.*\)/\1/g' -e 's/"$//g'`

echo NODE_NAME=$NODE_NAME
echo POD_NAME=$POD_NAME

. /bin/.load_keys

echo key download complete, starting prover

export DATABASE_URL=$PROVER_DATABASE_URL
exec prover 2>&1
