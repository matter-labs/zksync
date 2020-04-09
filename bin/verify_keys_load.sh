#!/bin/bash

. .setup_env

set -x
set -e

VERIFY_KEYS_TARBAL="verify-keys-`basename $KEY_DIR`-account-"$ACCOUNT_TREE_DEPTH"_-balance-$BALANCE_TREE_DEPTH.tar.gz"

[ ! -z $KEYS_SPACE_URL ] || (echo "KEYS_SPACE_URL is not set" && exit 1)

cd $ZKSYNC_HOME
curl -Sl $KEYS_SPACE_URL/$VERIFY_KEYS_TARBAL | tar xv
