#!/bin/bash

DEPLOY_STEP=${1:-"0"}

# Redeploy current contracts
# NOTE: this script does not build contracts, to build them use `zksync redeploy`

echo "redeploying for the db $DATABASE_URL"

update_env_file() {
  STEP=$1
  case $STEP in
  0)
    GOVERNANCE_TARGET_ADDR_NEW_VALUE=$(grep "GOVERNANCE_TARGET_ADDR" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $GOVERNANCE_TARGET_ADDR_NEW_VALUE
    ;;
  1)
    GOVERNANCE_GENESIS_TX_HASH_NEW_VALUE=$(grep "GOVERNANCE_GENESIS_TX_HASH" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $GOVERNANCE_GENESIS_TX_HASH_NEW_VALUE
    GOVERNANCE_ADDR_NEW_VALUE=$(grep "GOVERNANCE_ADDR" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $GOVERNANCE_ADDR_NEW_VALUE
    ;;
  2)
    VERIFIER_TARGET_ADDR_NEW_VALUE=$(grep "VERIFIER_TARGET_ADDR" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $VERIFIER_TARGET_ADDR_NEW_VALUE
    ;;
  3)
    VERIFIER_ADDR_NEW_VALUE=$(grep "VERIFIER_ADDR" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $VERIFIER_ADDR_NEW_VALUE
    ;;
  4)
    CONTRACT_TARGET_ADDR_NEW_VALUE=$(grep "CONTRACT_TARGET_ADDR" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $CONTRACT_TARGET_ADDR_NEW_VALUE
    ;;
  5)
    CONTRACT_GENESIS_TX_HASH_NEW_VALUE=$(grep "CONTRACT_GENESIS_TX_HASH" deploy.log)
    CONTRACT_ADDR_NEW_VALUE=$(grep "CONTRACT_ADDR" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $CONTRACT_GENESIS_TX_HASH_NEW_VALUE
    python3 bin/replace-env-variable.py ./$ENV_FILE $CONTRACT_ADDR_NEW_VALUE
    ;;
  6)
    UPGRADE_GATEKEEPER_ADDR_NEW_VALUE=$(grep "UPGRADE_GATEKEEPER_ADDR" deploy.log)
    python3 bin/replace-env-variable.py ./$ENV_FILE $UPGRADE_GATEKEEPER_ADDR_NEW_VALUE
    ;;
  *)
    ;;
  esac
}

for CURRENT_STEP in {0..8}; do
  if [ "$DEPLOY_STEP" -gt "$CURRENT_STEP" ]; then
    echo "Skipping step $CURRENT_STEP"
    continue
  fi
  echo "Started executing step $CURRENT_STEP"
  . .setup_env

  cd contracts
  yarn deploy-no-build --deployStep $CURRENT_STEP | tee ../deploy.log
  cd ..
  update_env_file $CURRENT_STEP
  echo "Finished executing $CURRENT_STEP"
done

exit 0
