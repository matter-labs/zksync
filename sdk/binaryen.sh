#!/bin/bash

# This file is used to build binaryen executables, specifically we need wasm2js.
# TODO: use CXX flags to speed up the build time and correctly set the number of workers.

BINARYEN=$ZKSYNC_HOME/sdk/binaryen

set -e

cd $BINARYEN
cmake . && make -j 8
cd -
