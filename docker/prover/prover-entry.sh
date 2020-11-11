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

# Returns required plonk setup powers based on `BLOCK_CHUNK_SIZES` used in the environment configuration
function get_required_plonk_setup_powers() {
   local SUP_CHUNKS_ARR=($(echo $SUPPORTED_BLOCK_CHUNKS_SIZES | tr ',' "\n"))
   local SUP_CHUNKS_POW=($(echo $SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS | tr ',' "\n"))

   local REQUIRED_SETUP_POWS=""
   for index in ${!SUP_CHUNKS_ARR[*]}; do
       for my_size in ${BLOCK_CHUNK_SIZES//,/ }; do
           if [ "$my_size" == "${SUP_CHUNKS_ARR[$index]}" ]; then
               REQUIRED_SETUP_POWS="$REQUIRED_SETUP_POWS${SUP_CHUNKS_POW[$index]},"
           fi
       done
   done
   echo $REQUIRED_SETUP_POWS
}

# we download only keys used in node (defined by $BLOCK_CHUNK_SIZES)
REQUIRED_SETUP_POWS=`get_required_plonk_setup_powers`
# install zk
echo installing zk
cd /infrastructure/zk && yarn install && yarn build

if [ "$PROVER_DOWNLOAD_SETUP" == "false" ]; then
  echo Downloading setup powers $REQUIRED_SETUP_POWS
  /bin/zk run plonk-setup $REQUIRED_SETUP_POWS
fi

/bin/zk run verify-keys unpack

echo key download complete, starting prover

exec plonk_step_by_step_prover "$PROVER_NAME" 2>&1
