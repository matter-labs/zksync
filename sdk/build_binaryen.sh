#!/bin/bash

set -e

# Build binaryen executables.

# path to binaryen
BINARYEN=$ZKSYNC_HOME/sdk/binaryen
# number of workers for make -j
CORES=$(grep -c ^processor /proc/cpuinfo 2> /dev/null || sysctl -n hw.ncpu 2> /dev/null || psrinfo -p)
# flags for cmake
CMAKE_FLAGS=-DCMAKE_BUILD_TYPE=Debug

git submodule update --init --recursive

if command -v clang &> /dev/null
then
    CMAKE_FLAGS=$CMAKE_FLAGS\ -DCMAKE_C_COMPILER=clang\ -DCMAKE_CXX_COMPILER=clang++
fi

if command -v lld &> /dev/null && [[ "$OSTYPE" != "darwin" ]]
then
    CMAKE_FLAGS=$CMAKE_FLAGS\ -DCMAKE_EXE_LINKER_FLAGS=-fuse-ld=lld
fi

cd $BINARYEN
cmake $CMAKE_FLAGS . && make -j $CORES
cd - &> /dev/null
