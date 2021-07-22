#!/bin/bash

set -e

export ZKSYNC_HOME="/"

PROVER_NAME=`hostname`
echo PROVER_NAME=$PROVER_NAME

echo CHAIN_CIRCUIT_SUPPORTED_BLOCK_CHUNKS_SIZES=$CHAIN_CIRCUIT_SUPPORTED_BLOCK_CHUNKS_SIZES
echo CHAIN_CIRCUIT_SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS=$CHAIN_CIRCUIT_SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS
echo CHAIN_STATE_KEEPER_BLOCK_CHUNK_SIZES=$CHAIN_STATE_KEEPER_BLOCK_CHUNK_SIZES


if [ "$MISC_DOCKER_DUMMY_PROVER" == "true" ]; then
  echo "Starting dummy_prover"
  exec dummy_prover "$PROVER_NAME" 2>&1
fi

# Returns required plonk setup powers based on `CHAIN_STATE_KEEPER_BLOCK_CHUNK_SIZES` used in the environment configuration
function get_required_plonk_setup_powers() {
   local SUP_CHUNKS_ARR=($(echo $CHAIN_CIRCUIT_SUPPORTED_BLOCK_CHUNKS_SIZES | tr ',' "\n"))
   local SUP_CHUNKS_POW=($(echo $CHAIN_CIRCUIT_SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS | tr ',' "\n"))

   local REQUIRED_SETUP_POWS=""
   for index in ${!SUP_CHUNKS_ARR[*]}; do
       for my_size in ${CHAIN_STATE_KEEPER_BLOCK_CHUNK_SIZES//,/ }; do
           if [ "$my_size" == "${SUP_CHUNKS_ARR[$index]}" ]; then
               REQUIRED_SETUP_POWS="$REQUIRED_SETUP_POWS${SUP_CHUNKS_POW[$index]},"
           fi
       done
   done
   echo $REQUIRED_SETUP_POWS
}

# we download only keys used in node (defined by $CHAIN_STATE_KEEPER_BLOCK_CHUNK_SIZES)
REQUIRED_SETUP_POWS=`get_required_plonk_setup_powers`

if [ "$PROVER_DOWNLOAD_SETUP" == "false" ]; then
  echo Downloading setup powers $REQUIRED_SETUP_POWS

  SETUP_DO_SPACE_DIR=https://universal-setup.ams3.digitaloceanspaces.com
  mkdir -p keys/setup && pushd keys/setup

  for i in ${REQUIRED_SETUP_POWS//,/ }; do
      axel -c $SETUP_DO_SPACE_DIR/setup_2%5E$i.key || true # don't download file if it is already there
      sleep 1 # to not receive "503 Slow Down"
  done

  popd
  echo Setup is downloaded
fi

VERIFY_KEYS_TARBAL="verify-keys-`basename $CHAIN_CIRCUIT_KEY_DIR`-account-"$CHAIN_CIRCUIT_ACCOUNT_TREE_DEPTH"_-balance-$CHAIN_CIRCUIT_BALANCE_TREE_DEPTH.tar.gz"

# checks if keys are present and if so, unpacks them
[ -f keys/packed/$VERIFY_KEYS_TARBAL ] || (echo Keys file $VERIFY_KEYS_TARBAL not found && exit 1)
tar xf keys/packed/$VERIFY_KEYS_TARBAL
echo Keys unpacked, starting prover

exec plonk_step_by_step_prover "$PROVER_NAME" 2>&1
