#!/bin/bash
 if [ "$#" -eq  "0" ]
   then
    echo "starting for rinkeby"
    export WEB3_URL=https://rinkeby.infura.io/48beda66075e41bda8b124c6a48fdfa0
    # export WEB3_URL=https://rinkeby.infura.io/v3/734de4d8205641beba7e48ec1a205c86
 else
    echo "starting local"
    export WEB3_URL=http://localhost:8545
fi
 
export SENDER_ACCOUNT=b4aaffeAAcb27098d9545A3C0e36924Af9EeDfe0
export PRIVATE_KEY=12B7678FF12FE8574AB74FFD23B5B0980B64D84345F9D637C2096CA0EF587806 

export CHAIN_ID=4

export CONTRACT_ADDR=3a0768b1302357033c83E4808D1C3F69f270c463
export FROM_BLOCK=3580765
export BELLMAN_VERBOSE=1
export RUST_BACKTRACE=1

cargo run --release --bin server
