#!/bin/bash

export ZKSYNC_HOME="/"

echo NODE_NAME=$NODE_NAME
echo POD_NAME=$POD_NAME

echo SUPPORTED_BLOCK_CHUNKS_SIZES=$SUPPORTED_BLOCK_CHUNKS_SIZES
echo SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS=$SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS
echo BLOCK_CHUNK_SIZES=$BLOCK_CHUNK_SIZES


# we donwload only keys used in node (defined by $BLOCK_CHUNK_SIZES)
SUP_CHUNKS_ARR=($(echo $SUPPORTED_BLOCK_CHUNKS_SIZES | tr ',' "\n"))
SUP_CHUNKS_POW=($(echo $SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS | tr ',' "\n"))

REQUIRED_SETUP_POWS=""
for index in ${!SUP_CHUNKS_ARR[*]}; do
    for my_size in ${BLOCK_CHUNK_SIZES//,/ }; do
        if [ $my_size == ${SUP_CHUNKS_ARR[$index]} ]; then
            REQUIRED_SETUP_POWS="$REQUIRED_SETUP_POS,${SUP_CHUNKS_POW[$index]}"
        fi
    done
done

echo Downloading setup powers $REQUIRED_SETUP_POWS

/bin/plonk-setup download monomial $REQUIRED_SETUP_POWS
/bin/verify-keys unpack

echo key download complete, starting prover

exec plonk_step_by_step_prover "$POD_NAME" 2>&1
