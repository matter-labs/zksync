#!/bin/bash

. .setup_env

FRANKLIN_HfOME=`dirname $0`/..

cp $FRANKLIN_HOME/contracts/build/Franklin.json $FRANKLIN_HOME/js/franklin_lib/abi
