#!/bin/bash
# This script downloads universal setup key files from our DO space
# You need setup files for proof generation, also it is used to generate verification keys for given circuit
#
# usage
# setup_keys_load [-lagrange]
# -lagrange - also download setup in lagrange form, off by default

SETUP_DO_SPACE_DIR=https://universal-setup.ams3.digitaloceanspaces.com


mkdir -p $ZKSYNC_HOME/keys/setup || exit 1
cd $ZKSYNC_HOME/keys/setup || exit 1

for i in {20..26}
do
    axel -c $SETUP_DO_SPACE_DIR/setup_2%5E$i.key

    sleep 1 # to not receive “503 Slow Down”
    if [ "$1" = "-lagrange" ]; then
        axel -c $SETUP_DO_SPACE_DIR/setup_2%5E"$i"_lagrange.key
    fi
    sleep 1 # to not receive “503 Slow Down”
done
