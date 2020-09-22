#!/bin/bash

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

. .setup_env

cd core/tests/ts-tests;
yarn --silent print-test-accounts | jq .
