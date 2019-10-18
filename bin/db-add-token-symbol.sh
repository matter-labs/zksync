#!/bin/bash

. .setup_env

set -e

TOKEN_ADDRESS=$1
SYMBOL=$2

echo Setting token $2 symbol to $1
psql "$DATABASE_URL" -c "UPDATE tokens \
                         SET symbol = '$SYMBOL' \
                         WHERE address = '$TOKEN_ADDRESS'"
