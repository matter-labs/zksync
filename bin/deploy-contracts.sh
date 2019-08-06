#!/bin/bash

. .setup_env

# // TODO key generation
# KEY_FILES=$CONTRACT_KEY_FILES
# .load_keys
#
# mkdir -p contracts/contracts/keys/
# cp -f $KEY_DIR/*.sol contracts/contracts/keys/

echo redeploying for the db $DATABASE_URL
cd contracts
yarn deploy  | tee ../deploy.log
cd ..

export LABEL=$FRANKLIN_ENV-`date +%Y-%m-%d-%H%M%S`

export NEW_CONTRACT=`cat deploy.log | grep "Franklin address" | grep -oE '0x(.+)' | sed -n "s/0x//p"`


if [[ ! -z "$NEW_CONTRACT" ]]
then
    echo New contract at $NEW_CONTRACT

    OLD_CONTRACT=`grep "^CONTRACT_ADDR" ./$ENV_FILE | grep -oE '=(.+)' | sed -n "s/=//p"`
    echo Old contract at $OLD_CONTRACT

    mkdir -p logs/$LABEL/
    cp deploy.log logs/$LABEL/deploy.log
    cp ./$ENV_FILE logs/$LABEL/$FRANKLIN_ENV.bak

    sed -i".bak" "s/^CONTRACT_ADDR=$OLD_CONTRACT/CONTRACT_ADDR=$NEW_CONTRACT/g" ./$ENV_FILE

    echo successfully deployed contracts

else
    echo "Contract deployment failed"
    exit 1
fi