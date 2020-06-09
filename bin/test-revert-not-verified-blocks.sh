#!/bin/bash

#$1 - main contract address

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

cd contracts;
yarn ts-node scripts/test-revert-not-verified-blocks.ts $1
