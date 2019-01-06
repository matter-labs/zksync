#!/bin/sh

ps auxw | grep target/release/server | grep -v grep > /dev/null

if [ $? != 0 ]
then
    export PATH="$HOME/.cargo/bin:$PATH"
    ./run.sh 2>&1 | tee /var/log/plasma.log
fi