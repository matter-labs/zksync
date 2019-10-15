#!/bin/bash

. .setup_env

rm -rf ./contracts/contracts/generated
mkdir -p ./contracts/contracts/generated
cp ./contracts/contracts/Governance.sol ./contracts/contracts/generated/GovernanceTest.sol
cp ./contracts/contracts/Franklin.sol ./contracts/contracts/generated/FranklinTest.sol
cp ./contracts/contracts/PriorityQueue.sol ./contracts/contracts/generated/PriorityQueueTest.sol
cp ./contracts/contracts/Verifier.sol ./contracts/contracts/generated/VerifierTest.sol

sed 's/Governance/GovernanceTest/' -i ./contracts/contracts/generated/GovernanceTest.sol

sed 's/.\/Bytes/..\/Bytes/' -i ./contracts/contracts/generated/FranklinTest.sol
sed 's/Governance/GovernanceTest/' -i ./contracts/contracts/generated/FranklinTest.sol
sed 's/Verifier/VerifierTest/' -i ./contracts/contracts/generated/FranklinTest.sol
sed 's/Franklin/FranklinTest/' -i ./contracts/contracts/generated/FranklinTest.sol
sed 's/PriorityQueue/PriorityQueueTest/' -i ./contracts/contracts/generated/FranklinTest.sol
sed 's/60/1/' -i ./contracts/contracts/generated/FranklinTest.sol
sed 's/100/1/' -i ./contracts/contracts/generated/FranklinTest.sol

sed 's/.\/Bytes/..\/Bytes/' -i ./contracts/contracts/generated/PriorityQueueTest.sol
sed 's/PriorityQueue/PriorityQueueTest/' -i ./contracts/contracts/generated/PriorityQueueTest.sol
sed 's/250/16/' -i ./contracts/contracts/generated/PriorityQueueTest.sol

sed 's/.\/VerificationKey/..\/VerificationKey/' -i ./contracts/contracts/generated/VerifierTest.sol
sed 's/Verifier/VerifierTest/' -i ./contracts/contracts/generated/VerifierTest.sol
sed 's/\/\/ Start/return true;/' -i ./contracts/contracts/generated/VerifierTest.sol
