#!/bin/bash

. .setup_env

# // TODO key generation
# KEY_FILES=$CONTRACT_KEY_FILES
# .load_keys
#
# mkdir -p contracts/contracts/keys/
# cp -f $KEY_DIR/*.sol contracts/contracts/keys/

echo "redeploying for the db $DATABASE_URL"
zksync flatten;
cd contracts;
yarn deploy | tee ../deploy.log;
cd ..;

CONTRACT_ADDR_NEW_VALUE=`grep "CONTRACT_ADDR" deploy.log`
ERC20_ADDR_NEW_VALUE=`grep "TEST_ERC20" deploy.log`
GOVERNANCE_ADDR_NEW_VALUE=`grep "GOVERNANCE_ADDR" deploy.log`
VERIFIER_ADDR_NEW_VALUE=`grep "VERIFIER_ADDR" deploy.log`
PRIORITY_QUEUE_ADDR_NEW_VALUE=`grep "PRIORITY_QUEUE_ADDR" deploy.log`
if [[ ! -z "$CONTRACT_ADDR_NEW_VALUE" ]]
then
    export LABEL=$ZKSYNC_ENV-Contract_deploy-`date +%Y-%m-%d-%H%M%S`
    mkdir -p logs/$LABEL/
    cp ./$ENV_FILE logs/$LABEL/$ZKSYNC_ENV.bak
    cp deploy.log logs/$LABEL/
    echo $CONTRACT_ADDR_NEW_VALUE
    python3 bin/replace-env-variable.py ./$ENV_FILE $CONTRACT_ADDR_NEW_VALUE
    python3 bin/replace-env-variable.py ./$ENV_FILE $ERC20_ADDR_NEW_VALUE
    python3 bin/replace-env-variable.py ./$ENV_FILE $GOVERNANCE_ADDR_NEW_VALUE
    python3 bin/replace-env-variable.py ./$ENV_FILE $VERIFIER_ADDR_NEW_VALUE
    python3 bin/replace-env-variable.py ./$ENV_FILE $PRIORITY_QUEUE_ADDR_NEW_VALUE
else
    echo "Contract deployment failed"
    exit 1
fi
