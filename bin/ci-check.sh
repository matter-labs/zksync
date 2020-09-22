#!/bin/bash
cd $ZKSYNC_HOME

set -e
set -x

function cargo_tests() {
    cargo fmt -- --check
    f cargo clippy --tests --benches -- -D warnings
    f cargo test
}


zksync init
pushd sdk/zksync-crypto
cargo_tests
popd
cargo_tests
zksync test-contracts
zksync circuit-tests
zksync prover-tests
zksync db-test
zksync integration-testkit
