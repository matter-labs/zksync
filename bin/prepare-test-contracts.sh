#!/bin/bash

. .setup_env

cd $ZKSYNC_HOME

IN_DIR=./contracts/contracts
OUT_DIR=./contracts/contracts/generated

rm -rf $OUT_DIR
mkdir -p $OUT_DIR
cp $IN_DIR/Governance.sol $OUT_DIR/GovernanceTest.sol
cp $IN_DIR/Verifier.sol $OUT_DIR/VerifierTest.sol
cp $IN_DIR/Franklin.sol $OUT_DIR/FranklinTest.sol
cp $IN_DIR/Storage.sol $OUT_DIR/StorageTest.sol
cp $IN_DIR/Config.sol $OUT_DIR/ConfigTest.sol
cp $IN_DIR/Ownable.sol $OUT_DIR/OwnableTest.sol
cp $IN_DIR/UpgradeMode.sol $OUT_DIR/UpgradeModeTest.sol
cp $IN_DIR/Upgradeable.sol $OUT_DIR/UpgradeableTest.sol
cp $IN_DIR/Proxy.sol $OUT_DIR/ProxyTest.sol

# Rename contracts
ssed 's/Governance/GovernanceTest/' -i $OUT_DIR/*.sol
ssed 's/Verifier/VerifierTest/' -i $OUT_DIR/*.sol
ssed 's/Franklin/FranklinTest/' -i $OUT_DIR/*.sol
ssed 's/Storage/StorageTest/' -i $OUT_DIR/*.sol
ssed 's/Config/ConfigTest/' -i $OUT_DIR/*.sol
ssed 's/Ownable/OwnableTest/' -i $OUT_DIR/*.sol
ssed 's/UpgradeMode/UpgradeModeTest/g' -i $OUT_DIR/*.sol
ssed 's/Upgradeable/UpgradeableTest/' -i $OUT_DIR/*.sol
ssed 's/Proxy/ProxyTest/' -i $OUT_DIR/*.sol
# Workaround -> ownable and upgradeable has Storage in comments.
ssed 's/StorageTest/Storage/' -i $OUT_DIR/OwnableTest.sol
ssed 's/StorageTest/Storage/' -i $OUT_DIR/UpgradeableTest.sol
# Workaround -> upgrade mode has UpgradeMode in event and variable names.
ssed 's/UpgradeModeTestEvents/UpgradeModeEvents/' -i $OUT_DIR/UpgradeModeTest.sol
ssed 's/UpgradeModeTestActivated/UpgradeModeActivated/' -i $OUT_DIR/UpgradeModeTest.sol
ssed 's/UpgradeModeTestFinalizeStatusActivated/UpgradeModeFinalizeStatusActivated/' -i $OUT_DIR/UpgradeModeTest.sol
ssed 's/UpgradeModeTestForciblyCanceled/UpgradeModeForciblyCanceled/' -i $OUT_DIR/UpgradeModeTest.sol
ssed 's/waitUpgradeModeTestActive/waitUpgradeModeActive/' -i $OUT_DIR/UpgradeModeTest.sol


# Changes solidity constant to provided value
# In solidity constant should be in the following form.
# $SOME_TYPE constant $NAME = $VALUE;
set_constant() {
	ssed -E "s/(.*constant $1)(.*)\;/\1 = $2\;/" -i $3
}
create_constant_getter() {
	ssed -E "s/    (.*) (constant $1)(.*)\;(.*)/    \1 \2\3\;\4\n    function get_$1() external pure returns (\1) {\n        return $1\;\n    }/" -i $2
}

# Change constants
set_constant MAX_AMOUNT_OF_REGISTERED_TOKENS 4 $OUT_DIR/ConfigTest.sol
set_constant EXPECT_VERIFICATION_IN 8 $OUT_DIR/ConfigTest.sol
set_constant MAX_UNVERIFIED_BLOCKS 4 $OUT_DIR/ConfigTest.sol
set_constant PRIORITY_EXPIRATION 16 $OUT_DIR/ConfigTest.sol
set_constant MAX_UPGRADE_PERIOD 5 $OUT_DIR/UpgradeModeTest.sol
set_constant WAIT_UPGRADE_MODE_PERIOD 3 $OUT_DIR/UpgradeModeTest.sol

create_constant_getter MAX_AMOUNT_OF_REGISTERED_TOKENS $OUT_DIR/ConfigTest.sol
create_constant_getter MAX_UPGRADE_PERIOD $OUT_DIR/UpgradeModeTest.sol
create_constant_getter WAIT_UPGRADE_MODE_PERIOD $OUT_DIR/UpgradeModeTest.sol

# Verify always true
set_constant DUMMY_VERIFIER true $OUT_DIR/VerifierTest.sol
