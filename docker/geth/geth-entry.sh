#!/bin/sh

cd /var/lib/geth/data

if [ ! -f ./keystore ]; then 
    echo initializing dev network
    cp /seed/dev.json ./
    cp /seed/password.sec ./
    geth --datadir . init dev.json
    cp /seed/keystore/UTC--2019-04-06T21-13-27.692266000Z--8a91dc2d28b689474298d91899f0c1baf62cb85b ./keystore/
fi

exec geth --networkid 9 --mine --minerthreads 1 \
    --datadir "." \
    --nodiscover \
    --rpc --rpcaddr "0.0.0.0" \
    --rpccorsdomain "*" --nat "any" --rpcapi eth,web3,personal,net \
    --unlock 0 --password "./password.sec" --allow-insecure-unlock \
    --ws --wsport 8546 \
    --gcmode archive \
    --wsorigins "*" --rpcvhosts=*
