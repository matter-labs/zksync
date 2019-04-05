#!/bin/bash

export SPACE_URL=https://ams3.digitaloceanspaces.com/keys
export KEY_FILES="deposit_pk.key exit_pk.key"

mkdir -p keys

for i in $KEY_FILES; do
    echo "Downloading $SPACE_URL/$i"
    if ! [ -f keys/$i ]; then
        curl -o keys/$i $SPACE_URL/$i
    fi
done

./prover