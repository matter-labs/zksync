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

cargo run --release --bin server
