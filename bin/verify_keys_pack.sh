#!/bin/bash
# Preapare keys in tarbar for publishing to DO spaces

. .setup_env

set -e
set -x

cd $ZKSYNC_HOME
VERIFY_KEYS_TARBAL="verify-keys-`basename $KEY_DIR`-account-"$ACCOUNT_TREE_DEPTH"_-balance-$BALANCE_TREE_DEPTH.tar.gz"
tar cvzf $VERIFY_KEYS_TARBAL $KEY_DIR
mkdir -p keys/packed
mv $VERIFY_KEYS_TARBAL keys/packed
