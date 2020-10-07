#!/bin/bash

. .setup_env

export DATABASE_URL=`echo $DATABASE_URL | ssed 's/plasma/plasma_test/g'`

cd core/lib/storage
if [ "$1" == "reset" ]; then
    diesel database reset
    diesel migration run
fi

cargo test --release -p zksync_storage --features "db_test" $2 -- --nocapture
