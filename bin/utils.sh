#!/bin/bash

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
