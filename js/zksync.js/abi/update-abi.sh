#!/bin/bash

cd `dirname $0`

cat $ZKSYNC_HOME/contracts/build/Franklin.json | jq '{ interface: .interface}' > SyncMain.json
cat $ZKSYNC_HOME/contracts/build/Governance.json | jq '{ interface: .interface}' > SyncGov.json
cat $ZKSYNC_HOME/contracts/build/PriorityQueue.json | jq '{ interface: .interface}' > SyncPriorityQueue.json
