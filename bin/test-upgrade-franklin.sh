#!/bin/bash

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

. .setup_env

cd contracts;
yarn ts-node scripts/test-upgrade-franklin.ts $1 $2
