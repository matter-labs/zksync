#!/bin/sh

ps auxw | grep target/release/server | grep -v grep > /dev/null

if [ $? != 0 ]
then
    ./run.sh 2>&1 | tee log.log
fi