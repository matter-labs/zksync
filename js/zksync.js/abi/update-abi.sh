#!/bin/bash

cd `dirname $0`

cat $ZKSYNC_HOME/contracts/build/ZkSync.json | jq '{ interface: .interface}' > SyncMain.json
cat $ZKSYNC_HOME/contracts/build/Governance.json | jq '{ interface: .interface}' > SyncGov.json
cat $ZKSYNC_HOME/contracts/build/IEIP1271.json | jq '{ interface: .interface}' > IEIP1271.json
