#!/bin/bash

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

. .setup_env

cd js/tests;
yarn print-test-accounts
