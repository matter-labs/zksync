#!/bin/bash

. .setup_env

IN_DIR=./contracts/contracts/
OUT_DIR=./contracts/contracts/generated

rm -rf $OUT_DIR
mkdir -p $OUT_DIR
cp $IN_DIR/Governance.sol $OUT_DIR/GovernanceTest.sol
cp $IN_DIR/Franklin.sol $OUT_DIR/FranklinTest.sol
cp $IN_DIR/PriorityQueue.sol $OUT_DIR/PriorityQueueTest.sol
cp $IN_DIR/Verifier.sol $OUT_DIR/VerifierTest.sol

sedi () {
    sed --version >/dev/null 2>&1 && sed -i -- -E "$@" || sed -i "" -E "$@"
}

# Rename contracts
sedi 's/Governance/GovernanceTest/' $OUT_DIR/*.sol
sedi 's/Franklin/FranklinTest/' $OUT_DIR/*.sol
sedi 's/PriorityQueue/PriorityQueueTest/' $OUT_DIR/*.sol
sedi 's/Verifier/VerifierTest/' $OUT_DIR/*.sol
# Workaround -> priority queue has FranklinTest in method names.
sedi 's/FranklinTest/Franklin/' $OUT_DIR/PriorityQueueTest.sol


# Changes solidity constant to provided value
# In solidity constant should be in the following form.
# $SOME_TYPE constant $NAME = $VALUE;
set_constant() {
	sedi "s/(.*constant $1)(.*)\;/\1 = $2\;/" $3
}

# Change constants
set_constant EXPECT_VERIFICATION_IN 8 $OUT_DIR/FranklinTest.sol
set_constant MAX_UNVERIFIED_BLOCKS 4 $OUT_DIR/FranklinTest.sol
set_constant PRIORITY_EXPIRATION 16 $OUT_DIR/PriorityQueueTest.sol

# Verify always true
set_constant DUMMY_VERIFIER true $OUT_DIR/VerifierTest.sol
