#!/bin/bash
cd $ZKSYNC_HOME

set -e
set -x

cargo fmt -- --check
zksync init
f cargo clippy --tests --benches -- -D warnings
f cargo test
zksync test-contracts
zksync circuit-tests
zksync prover-tests
zksync db-test
zksync integration-testkit