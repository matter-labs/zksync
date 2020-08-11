#!/bin/bash

cd `dirname $0`

cat $ZKSYNC_HOME/contracts/build/ZkSync.json | jq '{ abi: .abi}' > SyncMain.json
cat $ZKSYNC_HOME/contracts/build/Governance.json | jq '{ abi: .abi}' > SyncGov.json
cat $ZKSYNC_HOME/contracts/build/IEIP1271.json | jq '{ abi: .abi}' > IEIP1271.json
