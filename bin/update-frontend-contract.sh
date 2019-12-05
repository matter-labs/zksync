#!/bin/bash

. .setup_env

ZKSYNC_HOME=`dirname $0`/..

jq '{ interface: .interface }' $ZKSYNC_HOME/contracts/build/Franklin.json > $ZKSYNC_HOME/js/franklin_lib/abi/Franklin.json
jq '{ interface: .interface }' $ZKSYNC_HOME/contracts/build/Governance.json > $ZKSYNC_HOME/js/franklin_lib/abi/Governance.json
jq '{ interface: .interface }' $ZKSYNC_HOME/contracts/build/PriorityQueue.json > $ZKSYNC_HOME/js/franklin_lib/abi/PriorityQueue.json
