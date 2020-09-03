#!/bin/bash

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

set -e

cd contracts;

yarn --silent deploy-dev-erc20 --name $1 --symbol $2 --decimals $3
