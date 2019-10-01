#!/usr/bin/python3

import shutil
import subprocess
import os

os.mkdir("./contracts/contracts/generated")

shutil.copy2('./contracts/contracts/Governance.sol', './contracts/contracts/generated/GovernanceTest.sol')
subprocess.call(["sed", "-i", "", 's/Governance/GovernanceTest/', "./contracts/contracts/generated/GovernanceTest.sol"])

shutil.copy2('./contracts/contracts/Franklin.sol', './contracts/contracts/generated/FranklinTest.sol')

subprocess.call(["sed", "-i", "", 's/Franklin/FranklinTest/', "./contracts/contracts/generated/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/Governance/GovernanceTest/', "./contracts/contracts/generated/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/Verifier/VerifierTest/', "./contracts/contracts/generated/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/PriorityQueue/PriorityQueueTest/', "./contracts/contracts/generated/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/60/1/', "./contracts/contracts/generated/FranklinTest.sol"])
subprocess.call(["sed", "-i", "", 's/100/1/', "./contracts/contracts/generated/FranklinTest.sol"])

shutil.copy2('./contracts/contracts/PriorityQueue.sol', './contracts/contracts/generated/PriorityQueueTest.sol')

subprocess.call(["sed", "-i", "", 's/PriorityQueue/PriorityQueueTest/', "./contracts/contracts/generated/PriorityQueueTest.sol"])
subprocess.call(["sed", "-i", "", 's/250/16/', "./contracts/contracts/generated/PriorityQueueTest.sol"])