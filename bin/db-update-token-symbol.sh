#!/bin/bash

# Usage: db-update-token-symbol.sh token_address new_token_symbol

zk db update token $1 $2
