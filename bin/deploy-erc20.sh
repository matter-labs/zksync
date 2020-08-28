#!/bin/bash

if [ ! -z $ZKSYNC_HOME ]
then
  cd $ZKSYNC_HOME
fi

set -e

if [[ $1 == "test" ]]
then 
  export ETH_NETWORK="test"
fi

cd contracts;

NAME=$2 SYMBOL=$3 DECIMALS=$4 yarn --silent deploy-dev-erc20
