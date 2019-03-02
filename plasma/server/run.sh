#!/bin/bash
if [ "$#" -eq  "0" ]
then
   echo "starting for rinkeby"
else
   echo "starting local"
   export WEB3_URL=http://localhost:8545
fi

LOGFILE=/var/log/plasma.log
cargo run --release --bin server | tee ${LOG_FILE}
