# Core Components Changelog

All notable changes to the core components will be documented in this file.

## Unreleased

- Removed the limitation on the number of withdrawals in the block.
- (`FeeTicker`): Increased gas price estimate for transaction.

### Removed

- `MetricsCounter` structure was removed because it is not used.

### Changed

- Type aliases (`TokenId`, `AccountId`, `Nonce`, `BlockNumber`, `PriorityOpId`, `EthBlockId`) are replaced with wrapper
  structures.
- `prometheus_exporter` was made a library to be used by several crates.
- `prover_run_for_next_commit` function uses a parameterized timeout instead of a hard-coded one.

### Added

- `prometheus_exporter` is launched by every microservice.
- `tokens_acceptable_for_fees` endpoint was added to REST API v0.1.

### Fixed

## Release 2021-01-12

### Changed

- `gen_token_add_contract` crate is rewritten in ts.
- Metrics were added to some functions from lib/storage.
- `get_tx_by_hash` function was simplified.

### Added

- `closest_greater_or_eq_packable_fee_amount` and `closest_greater_or_eq_packable_token_amount` functions.
  `test_float_conversions` test was expanded.
- Loadtest scenario for stressing transaction batches

### Removed

- Sequential Sparse Merkle Tree implementation was removed because it has been replaced by the parallel implementation.

### Fixed

- Bug with `to_float` function. Now, it really rounds to the closest less or equal float number.
- Wrong index type used in the database causing some queries to take too much time.

## Prior to 2020-12-23

### Added

- A possibility to get an Ethereum tx hash for withdrawal operation.
- Support for non-standard Ethereum signatures.

### Changed

- Robustness of the fee ticker's API interacting module was increased.
- Blocks that contain withdraw operations are sealed faster.
- `eth_sender` module now can be disabled.
- Transfer to zero address (0x00..00) is now forbidden in zkSync.
- WebSocket server now uses more threads for handling incoming requests.

### Fixed

- Bug with delay between receiving a job and starting sending heartbeats.
