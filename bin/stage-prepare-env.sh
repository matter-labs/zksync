#!/bin/bash

cp $ZKSYNC_HOME/etc/env/{dev.env.example,stage.env}
STAGE_ENV_FILE=$ZKSYNC_HOME/etc/env/stage.env
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE SERVER_API_HOST=stage-api.zksync.dev
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE REST_API_ADDR=https://stage-api.zksync.dev
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE HTTP_RPC_API_ADDR=https://stage-api.zksync.dev/jsrpc
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE WS_API_ADDR=wss://stage-api.zksync.dev/jsrpc-ws
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE ETH_NETWORK=rinkeby
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE DATABASE_URL=$1
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE WEB3_URL=$2
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE API_SERVER=https://stage-api.zksync.dev
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE BLOCK_SIZE_CHUNKS=50
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE ACCOUNT_TREE_DEPTH=16
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE ZKSYNC_ACTION=dont_ask
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE PROVER_SERVER_URL=http://stage-server:8088
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE KEYS_SPACE_URL=https://keys-multitoken.fra1.digitaloceanspaces.com/stage
python3 $ZKSYNC_HOME/bin/replace-env-variable.py $STAGE_ENV_FILE CONTRACT_ADDR=$3

zksync env stage
