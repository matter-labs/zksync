#!/bin/sh

cd /var/lib/geth/data

if [ ! -f ./dev.json ]; then cp /seed/dev.json ./; fi
if [ ! -f ./password.sec ]; then cp /seed/password.sec ./; fi
if [ ! -f ./keystore/UTC--2019-04-06T21-13-27.692266000Z--8a91dc2d28b689474298d91899f0c1baf62cb85b ]; then cp /seed/keystore/UTC--2019-04-06T21-13-27.692266000Z--8a91dc2d28b689474298d91899f0c1baf62cb85b ./keystore/; fi

geth --networkid 9 --mine --minerthreads 1 \
    --datadir "." \
    --nodiscover \
    --rpc --rpcaddr "0.0.0.0" \
    --rpccorsdomain "*" --nat "any" --rpcapi eth,web3,personal,net \
    --unlock 0 --password "./password.sec" --allow-insecure-unlock \
    --ws --wsport 8546 \
    --wsorigins "*" --rpcvhosts=*
