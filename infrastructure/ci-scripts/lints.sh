zk fmt --check
yarn lint:md
yarn lint:sol
zk dummy-prover ensure-disabled
ulimit -S -c unlimited
cargo fmt --all -- --check
zk f cargo clippy --tests --benches -- -D warnings
cd sdk/zksync-crypto
cargo fmt -- --check
cargo clippy --all --tests --benches -- -D warnings
