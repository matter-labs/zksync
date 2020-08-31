#!/bin/bash

. .setup_env

cd $ZKSYNC_HOME

IN_DIR=./contracts/contracts
OUT_DIR=./contracts/dev-contracts/generated

rm -rf $OUT_DIR
mkdir -p $OUT_DIR
cp $IN_DIR/Governance.sol $OUT_DIR/GovernanceTest.sol
cp $IN_DIR/Verifier.sol $OUT_DIR/VerifierTest.sol
cp $IN_DIR/ZkSync.sol $OUT_DIR/ZkSyncTest.sol
cp $IN_DIR/Storage.sol $OUT_DIR/StorageTest.sol
cp $IN_DIR/Config.sol $OUT_DIR/ConfigTest.sol
cp $IN_DIR/UpgradeGatekeeper.sol $OUT_DIR/UpgradeGatekeeperTest.sol
cp $IN_DIR/ZkSync.sol $OUT_DIR/ZkSyncTestUpgradeTarget.sol

# Rename contracts
ssed 's/Governance/GovernanceTest/' -i $OUT_DIR/*.sol
ssed 's/\bVerifier\b/VerifierTest/' -i $OUT_DIR/*.sol
ssed 's/ZkSync/ZkSyncTest/' -i $OUT_DIR/*.sol
ssed 's/Storage/StorageTest/' -i $OUT_DIR/*.sol
ssed 's/Config/ConfigTest/' -i $OUT_DIR/*.sol
ssed 's/UpgradeGatekeeper/UpgradeGatekeeperTest/' -i $OUT_DIR/*.sol

# Renaming of ZkSyncTestUpgradeTarget contract
ssed 's/contract ZkSyncTest/contract ZkSyncTestUpgradeTarget/' -i $OUT_DIR/ZkSyncTestUpgradeTarget.sol


# Changes solidity constant to provided value
# In solidity constant should be in the following form.
# $SOME_TYPE constant $NAME = $VALUE;
set_constant() {
	ssed -E "s/(.*constant $1 =)(.*)\;/\1 $2\;/" -i $3
}
create_constant_getter() {
	ssed -E "s/    (.*) (constant $1 =)(.*)\;(.*)/    \1 \2\3\;\4\n    function get_$1() external pure returns (\1) {\n        return $1\;\n    }/" -i $2
}

# Change constants
set_constant MAX_AMOUNT_OF_REGISTERED_TOKENS 5 $OUT_DIR/ConfigTest.sol
set_constant EXPECT_VERIFICATION_IN 8 $OUT_DIR/ConfigTest.sol
set_constant MAX_UNVERIFIED_BLOCKS 4 $OUT_DIR/ConfigTest.sol
set_constant PRIORITY_EXPIRATION 101 $OUT_DIR/ConfigTest.sol
set_constant UPGRADE_NOTICE_PERIOD 4 $OUT_DIR/ConfigTest.sol

create_constant_getter MAX_AMOUNT_OF_REGISTERED_TOKENS $OUT_DIR/ConfigTest.sol
create_constant_getter EXPECT_VERIFICATION_IN $OUT_DIR/ConfigTest.sol
create_constant_getter UPGRADE_NOTICE_PERIOD $OUT_DIR/UpgradeGatekeeperTest.sol

# Verify always true
set_constant DUMMY_VERIFIER true $OUT_DIR/VerifierTest.sol

# Make upgrade function in ZkSyncTestUpgradeTarget contract to do nothing
ssed -E "s/revert\(\"upgzk\"\);(.*)/\/\*revert\(\"upgzk\"\);\*\/\1/" -i $OUT_DIR/ZkSyncTestUpgradeTarget.sol
