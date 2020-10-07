#!/bin/bash

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

cd contracts;
yarn deploy-testkit $@
