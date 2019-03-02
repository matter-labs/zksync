#!/bin/sh

# This script shall be used on on server; it starts run.sh internally

ps auxw | grep target/release/server | grep -v grep > /dev/null

if [ $? != 0 ]
then
    cp /var/log/plasma.log plasma-`date +%Y-%m-%d-%H%M%S`.log

    export PATH="$HOME/.cargo/bin:$PATH"
    ./run.sh 2>&1 | tee /var/log/plasma.log
fi