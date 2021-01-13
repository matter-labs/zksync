#!/bin/bash

cd `dirname $0`

cat $ZKSYNC_HOME/contracts/artifacts/cache/solpp-generated-contracts/ZkSync.sol/ZkSync.json | jq '{ abi: .abi}' > SyncMain.json
cat $ZKSYNC_HOME/contracts/artifacts/cache/solpp-generated-contracts/Governance.sol/Governance.json | jq '{ abi: .abi}' > SyncGov.json
