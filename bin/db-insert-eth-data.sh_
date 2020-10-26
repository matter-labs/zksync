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
    --data '{"jsonrpc":"2.0","method":"eth_getTransactionCount","params":['"\"$OPERATOR_COMMIT_ETH_ADDRESS\""',"pending"],"id":1}' \
    $WEB3_URL 2> /dev/null \
    | jq '.result'`

# Strip quotes around the nonce value. Result will be like `0x123`.
eval NONCE=$NONCE

# Convert the number from the hexadecimal form to the decimal. The result will be like `291`.
NONCE=`printf "%d\n" $NONCE`

# Insert data: nonce (obtained above), gas price limit (obtained from env), stats data (defaults to zero)
psql "$DATABASE_URL" -c "INSERT INTO eth_parameters (nonce, gas_price_limit, commit_ops, verify_ops, withdraw_ops) \
                         VALUES ('$NONCE', '$ETH_GAS_PRICE_DEFAULT_LIMIT', 0, 0, 0) \
                         ON CONFLICT (id) DO UPDATE  \
                         SET (commit_ops, verify_ops, withdraw_ops) = (0, 0, 0)" || exit 1

echo "inserted Ethereum nonce ($NONCE)"
echo "successfully initialized the Ethereum parameters table"
