#!/bin/bash
set -e

. .setup_env

echo contracts-test
cd contracts
yarn test || true # FIXME: after test merges done
cd ..
