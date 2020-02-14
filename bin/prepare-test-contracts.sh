#!/bin/bash

. .setup_env

cd $ZKSYNC_HOME

IN_DIR=./contracts/contracts
OUT_DIR=./contracts/contracts/generated

rm -rf $OUT_DIR
mkdir -p $OUT_DIR
cp $IN_DIR/Governance.sol $OUT_DIR/GovernanceTest.sol
cp $IN_DIR/PriorityQueue.sol $OUT_DIR/PriorityQueueTest.sol
cp $IN_DIR/Verifier.sol $OUT_DIR/VerifierTest.sol
cp $IN_DIR/Franklin.sol $OUT_DIR/FranklinTest.sol
cp $IN_DIR/Storage.sol $OUT_DIR/StorageTest.sol

# Rename contracts
ssed 's/Governance/GovernanceTest/' -i $OUT_DIR/*.sol
ssed 's/Franklin/FranklinTest/' -i $OUT_DIR/*.sol
ssed 's/Storage/StorageTest/' -i $OUT_DIR/*.sol
ssed 's/PriorityQueue/PriorityQueueTest/' -i $OUT_DIR/*.sol
ssed 's/Verifier/VerifierTest/' -i $OUT_DIR/*.sol
# Workaround -> priority queue has FranklinTest in method names.
ssed 's/FranklinTest/Franklin/' -i $OUT_DIR/PriorityQueueTest.sol


# Changes solidity constant to provided value
# In solidity constant should be in the following form.
# $SOME_TYPE constant $NAME = $VALUE;
set_constant() {
	ssed -E "s/(.*constant $1)(.*)\;/\1 = $2\;/" -i $3
}

# Change constants
set_constant EXPECT_VERIFICATION_IN 8 $OUT_DIR/FranklinTest.sol
set_constant MAX_UNVERIFIED_BLOCKS 4 $OUT_DIR/FranklinTest.sol
set_constant PRIORITY_EXPIRATION 16 $OUT_DIR/PriorityQueueTest.sol

# Verify always true
set_constant DUMMY_VERIFIER true $OUT_DIR/VerifierTest.sol
