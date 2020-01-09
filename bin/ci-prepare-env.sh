#!/bin/bash

cp $ZKSYNC_HOME/etc/env/{dev.env.example,ci.env}
CI_ENV_FILE=$ZKSYNC_HOME/etc/env/ci.env
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE DATABASE_URL=postgres://postgres@postgres/plasma
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE PROVER_DATABASE_URL=postgres://postgres@postgres/plasma
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE WEB3_URL=http://geth:8545

zksync env ci
zksync gen-keys-if-not-present
