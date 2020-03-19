#!/bin/bash

# Force read env -- this is important, sp that we re-ready the new contract value after redeploy!!!
ZKSYNC_ENV=
. .setup_env

# Retrieve pending nonce from the node and obtain the value via `jq`.
# NONCE variable will have the value like `"0x123"`.
# Log output is redirected to the `/dev/null` to avoid garbage in the overall command output.
NONCE=`curl \
    -H "Accept: application/json" \
    -H "Content-Type: application/json" \
    -X POST \
    --data '{"jsonrpc":"2.0","method":"eth_getTransactionCount","params":['"\"$OPERATOR_ETH_ADDRESS\""',"pending"],"id":1}' \
    $WEB3_URL 2> /dev/null \
    | jq '.result'`

# Strip quotes around the nonce value. Result will be like `0x123`.
eval NONCE=$NONCE

# Convert the number from the hexadecimal form to the decimal. The result will be like `291`.
NONCE=`printf "%d\n" $NONCE`


psql "$DATABASE_URL" -c "INSERT INTO eth_nonce (nonce) \
                         VALUES ('$NONCE') \
                         ON CONFLICT (id) DO UPDATE  \
                         SET nonce = '$NONCE'" || exit 1
echo "successfully inserted the Ethereum nonce ($NONCE) into the database"