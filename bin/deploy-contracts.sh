#!/bin/bash

set -e
# Redeploy current contracts
# NOTE: this script does not build contracts, to build them use `zksync redeploy`

echo "Redeploying contracts, results will be inserted to the db $DATABASE_URL"


replace_env_variable() {
  pushd $ZKSYNC_HOME > /dev/null
  python3 bin/replace-env-variable.py ./$ENV_FILE $(grep "$1" deploy.log)
  . .setup_env
  popd > /dev/null
}

cd contracts/
yarn deploy-no-build | tee ../deploy.log
replace_env_variable "GOVERNANCE_TARGET_ADDR"
replace_env_variable "VERIFIER_TARGET_ADDR"
replace_env_variable "CONTRACT_TARGET_ADDR"
replace_env_variable "GOVERNANCE_ADDR"
replace_env_variable "CONTRACT_ADDR"
replace_env_variable "VERIFIER_ADDR"
replace_env_variable "GATEKEEPER_ADDR"
replace_env_variable "DEPLOY_FACTORY_ADDR"
replace_env_variable "GENESIS_TX_HASH"
