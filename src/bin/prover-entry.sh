#!/bin/sh

mkdir -p keys

for i in $KEY_FILES; do
    echo "Downloading $SPACE_URL/$i"
    if ! [ -f keys/$i ]; then
        curl -o keys/$i $SPACE_URL/$i
    fi
done

echo key download complete, starting prover

echo DATABASE_URL=$DATABASE_URL
echo PROVER_DATABASE_URL=$PROVER_DATABASE_URL

export DATABASE_URL=$PROVER_DATABASE_URL
./prover
