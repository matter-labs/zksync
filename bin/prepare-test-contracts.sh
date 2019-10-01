#!/bin/bash

. .setup_env

mkdir -p ./contracts/contracts/generated
cp ./contracts/contracts/Governance.sol ./contracts/contracts/generated/GovernanceTest.sol
cp ./contracts/contracts/Franklin.sol ./contracts/contracts/generated/FranklinTest.sol
cp ./contracts/contracts/PriorityQueue.sol ./contracts/contracts/generated/PriorityQueueTest.sol

sed -i '' 's/Governance/GovernanceTest/' ./contracts/contracts/generated/GovernanceTest.sol

sed -i '' 's/Governance/GovernanceTest/' ./contracts/contracts/generated/FranklinTest.sol
sed -i '' 's/Verifier/VerifierTest/' ./contracts/contracts/generated/FranklinTest.sol
sed -i '' 's/Franklin/FranklinTest/' ./contracts/contracts/generated/FranklinTest.sol
sed -i '' 's/PriorityQueue/PriorityQueueTest/' ./contracts/contracts/generated/FranklinTest.sol
sed -i '' 's/60/1/' ./contracts/contracts/generated/FranklinTest.sol
sed -i '' 's/100/1/' ./contracts/contracts/generated/FranklinTest.sol

sed -i '' 's/PriorityQueue/PriorityQueueTest/' ./contracts/contracts/generated/PriorityQueueTest.sol
sed -i '' 's/250/16/' ./contracts/contracts/generated/PriorityQueueTest.sol