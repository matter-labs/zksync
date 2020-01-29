#!/bin/bash

set -e

check_tool() {
    command -v $1 > /dev/null || (echo $1 not found && exit 1)
}

echo Checking environment

check_tool yarn
check_tool node
node --version | grep "v10.*" > /dev/null  || (echo "ERROR, need node version 10" && exit 1)
check_tool docker
check_tool docker-compose
check_tool envsubst
check_tool cargo
check_tool jq
check_tool psql
check_tool pg_isready
check_tool diesel

( (sed --version | grep GNU) &> /dev/null || (gsed --version | grep GNU) &> /dev/null ) || (echo "sed or gsed should be GNU-sed" && exit 1)

echo Environment is fine
