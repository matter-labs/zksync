#!/bin/bash

. .setup_env

for ((i=0;i<=$TEST_WALLETS_TOTAL-1;i++)); do
    cd js/tests && WALLET=$i yarn loadtest &
done