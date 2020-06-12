#!/bin/bash

export ZKSYNC_HOME="/"

PROVER_NAME=`hostname`
echo PROVER_NAME=$PROVER_NAME

echo SUPPORTED_BLOCK_CHUNKS_SIZES=$SUPPORTED_BLOCK_CHUNKS_SIZES
echo SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS=$SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS
echo BLOCK_CHUNK_SIZES=$BLOCK_CHUNK_SIZES


if [ "$DOCKER_DUMMY_PROVER" == "true" ]; then
  echo "Starting dummy_prover"
  exec dummy_prover "$PROVER_NAME" 2>&1
fi

# we download only keys used in node (defined by $BLOCK_CHUNK_SIZES)
source /bin/utils.sh
REQUIRED_SETUP_POWS=`get_required_plonk_setup_powers`

if [ "$PROVER_DOWNLOAD_SETUP" == "false" ]; then
  echo Downloading setup powers $REQUIRED_SETUP_POWS
  /bin/plonk-setup download monomial $REQUIRED_SETUP_POWS
fi

/bin/verify-keys unpack

echo key download complete, starting prover

exec plonk_step_by_step_prover "$PROVER_NAME" 2>&1
