#!/bin/bash

. .setup_env

FRANKLIN_HOME=`dirname $0`/..

jq '{ interface: .interface }' $FRANKLIN_HOME/contracts/build/Franklin.json > $FRANKLIN_HOME/js/franklin_lib/abi/Franklin.json
jq '{ interface: .interface }' $FRANKLIN_HOME/contracts/build/PriorityQueue.json > $FRANKLIN_HOME/js/franklin_lib/abi/PriorityQueue.json
