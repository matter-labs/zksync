#!/bin/bash

cd `dirname $0`

# Copying contract 
cat $ZKSYNC_HOME/contracts/artifacts/cache/solpp-generated-contracts/ZkSync.sol/ZkSync.json | jq '{ abi: .abi}' > SyncMain.json
cat $ZKSYNC_HOME/contracts/artifacts/cache/solpp-generated-contracts/Governance.sol/Governance.json | jq '{ abi: .abi}' > SyncGov.json
cat $ZKSYNC_HOME/contracts/artifacts/cache/solpp-generated-contracts/Governance.sol/ZkSyncNFTFactory.json | jq '{ abi: .abi}' > ZkSyncNFTFactory.json

cp $ZKSYNC_HOME/contracts/typechain/Governance.d.ts ../src/typechain/Governance.d.ts
cp $ZKSYNC_HOME/contracts/typechain/GovernanceFactory.ts ../src/typechain/GovernanceFactory.ts

cp $ZKSYNC_HOME/contracts/typechain/ZkSync.d.ts ../src/typechain/ZkSync.d.ts
cp $ZKSYNC_HOME/contracts/typechain/ZkSyncFactory.ts ../src/typechain/ZkSyncFactory.ts

cp $ZKSYNC_HOME/contracts/typechain/ZkSyncNFTFactory.d.ts ../src/typechain/ZkSyncNFTFactoryd.d.ts
cp $ZKSYNC_HOME/contracts/typechain/ZkSyncNFTFactoryFactory.ts ../src/typechain/ZkSyncNFTFactoryFactory.ts
