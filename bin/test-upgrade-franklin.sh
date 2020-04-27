#!/bin/bash

#$1 - main contract address
#$2 - gatekeeper contract address

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

cd contracts;
yarn ts-node scripts/test-upgrade-franklin.ts $1 $2
