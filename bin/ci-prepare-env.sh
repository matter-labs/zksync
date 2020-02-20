#!/bin/bash

cp $ZKSYNC_HOME/etc/env/{dev.env.example,ci.env}
CI_ENV_FILE=$ZKSYNC_HOME/etc/env/ci.env
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE DATABASE_URL=postgres://postgres@postgres/plasma
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE WEB3_URL=http://geth:8545
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE ZKSYNC_ACTION=dont_ask
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE REST_API_ADDR=http://start-server-detached:3000
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE HTTP_RPC_API_ADDR=http://start-server-detached:3030
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE WS_API_ADDR=http://start-server-detached:3031
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE API_SERVER=http://start-server-detached:3000

zksync env ci
zksync gen-keys-if-not-present
