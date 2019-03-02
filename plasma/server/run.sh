#!/bin/bash
if [ "$#" -eq  "0" ]
then
   echo "starting for rinkeby"
   export WEB3_URL=https://rinkeby.infura.io/48beda66075e41bda8b124c6a48fdfa0
else
   echo "starting local"
   export WEB3_URL=http://localhost:8545
fi

LOGFILE=/var/log/plasma.log
cargo run --release --bin server | tee ${LOG_FILE}
