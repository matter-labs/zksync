#!/bin/sh

echo NODE_NAME=$NODE_NAME
echo POD_NAME=$POD_NAME

. /bin/.load_keys

echo key download complete, starting prover

export DATABASE_URL=$PROVER_DATABASE_URL
./prover
