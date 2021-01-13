#!/bin/sh

zk fmt --check
ulimit -S -c unlimited
cargo fmt --all -- --check
zk f cargo clippy --tests --benches -- -D warnings
#cd sdk/zksync-crypto && cargo fmt -- --check
#cd sdk/zksync-crypto && cargo clippy --all --tests --benches -- -D warnings
