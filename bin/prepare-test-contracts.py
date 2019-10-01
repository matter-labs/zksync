#!/usr/bin/python3

import shutil
import subprocess

shutil.copy2('./contracts/contracts/Governance.sol', './contracts/contracts/GovernanceTest.sol')
subprocess.call(["sed", "-i", "", 's/Governance/GovernanceTest/', "./contracts/contracts/GovernanceTest.sol"])

shutil.copy2('./contracts/contracts/Franklin.sol', './contracts/contracts/FranklinTest.sol')

subprocess.call(["sed", "-i", "", 's/Franklin/FranklinTest/', "./contracts/contracts/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/Governance/GovernanceTest/', "./contracts/contracts/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/Verifier/VerifierTest/', "./contracts/contracts/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/PriorityQueue/PriorityQueueTest/', "./contracts/contracts/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/60/1/', "./contracts/contracts/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/100/1/', "./contracts/contracts/FranklinTest.sol"])


shutil.copy2('./contracts/contracts/PriorityQueue.sol', './contracts/contracts/PriorityQueueTest.sol')

subprocess.call(["sed", "-i", "", 's/PriorityQueue/PriorityQueueTest/', "./contracts/contracts/PriorityQueueTest.sol"])
subprocess.call(["sed", "-i", "", 's/250/16/', "./contracts/contracts/PriorityQueueTest.sol"])