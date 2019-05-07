#!/bin/sh

. /bin/.load_keys

echo key download complete, starting prover

export DATABASE_URL=$PROVER_DATABASE_URL
./prover
