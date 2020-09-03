#!/bin/bash
governance-add-erc20.sh 0xfab46e002bbf0b4509813474841e0716e6730136
echo waiting for token to get to db. accepted output is UPDATE 1
sleep 300
db-update-token-symbol.sh 0xfab46e002bbf0b4509813474841e0716e6730136 FAU
