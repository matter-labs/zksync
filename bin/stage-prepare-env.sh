#!/bin/bash

cp $ZKSYNC_HOME/etc/env/{dev.env.example,stage.env}
CI_ENV_FILE=$ZKSYNC_HOME/etc/env/stage.env
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE SERVER_API_HOST=stage-api.zksync.dev
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE REST_API_ADDR=https://stage-api.zksync.dev
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE HTTP_RPC_API_ADDR=https://stage-api.zksync.dev/jsrpc
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE WS_API_ADDR=wss://stage-api.zksync.dev/jsrpc-ws
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE ETH_NETWORK=rinkeby
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE DATABASE_URL=postgresql://doadmin:k6x0vbj4vn27i25h@staging-db-postgresql-ams3-22673-do-user-5048583-0.db.ondigitalocean.com:25060/defaultdb?sslmode=require
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE PROVER_DATABASE_URL=postgresql://doadmin:k6x0vbj4vn27i25h@staging-db-postgresql-ams3-22673-do-user-5048583-0.db.ondigitalocean.com:25060/defaultdb?sslmode=require
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE WEB3_URL=https://rinkeby.infura.io/v3/48beda66075e41bda8b124c6a48fdfa0
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE API_SERVER=https://stage-api.zksync.dev
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE BLOCK_SIZE_CHUNKS=18
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE ACCOUNT_TREE_DEPTH=16
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $CI_ENV_FILE ZKSYNC_ACTION=dont_ask

zksync env stage
