#!/bin/bash

. .setup_env

echo contracts-test
cd contracts
yarn test  | tee ../test.log
cd ..
