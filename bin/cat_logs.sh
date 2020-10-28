#!/bin/bash

echo "Server logs:"
echo "============"
cat $ZKSYNC_HOME/server.log
echo ""

echo "Prover logs:"
echo "============"
cat $ZKSYNC_HOME/dummy_prover.log
echo ""

# If we're calling this script, previous command failed and we want to exit with an error code 
exit 1
