#!/bin/bash

# Redeploy current contracts
# NOTE: this script does not build contracts, to build them use `zksync redeploy`

. .setup_env

cd contracts/
yarn publish-sources
