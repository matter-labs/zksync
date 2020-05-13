#!/bin/bash

export ZKSYNC_HOME="/"

echo NODE_NAME=$NODE_NAME
echo POD_NAME=$POD_NAME

echo SUPPORTED_BLOCK_CHUNKS_SIZES=$SUPPORTED_BLOCK_CHUNKS_SIZES
echo SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS=$SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS
echo BLOCK_CHUNK_SIZES=$BLOCK_CHUNK_SIZES


# we download only keys used in node (defined by $BLOCK_CHUNK_SIZES)
source /bin/utils.sh
REQUIRED_SETUP_POWS=`get_required_plonk_setup_powers`

echo Downloading setup powers $REQUIRED_SETUP_POWS


/bin/plonk-setup download monomial $REQUIRED_SETUP_POWS
# key dir is mounted as volume on kubernetes, so we have to copy packed keys from somewhere else
rm -rf $ZKSYNC_HOME/keys/packed
mv /keys-packed $ZKSYNC_HOME/keys/packed
/bin/verify-keys unpack

echo key download complete, starting prover

exec plonk_step_by_step_prover "$POD_NAME" 2>&1
