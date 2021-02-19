# Core Components Changelog

All notable changes to the core components will be documented in this file.

## Unreleased

### Removed

### Changed

### Added

### Fixed

## Release 2021-02-19

### Removed

### Changed

- The token name is now set for each scenario separately instead of the network section of the loadtest configuration.
- Rejected transactions are now stored in the database for 2 weeks only.

### Added

- Added a stressing dev fee ticker scenario to the loadtest.
- Added a `--sloppy` mode to the `dev-fee-ticker-server` to simulate bad networks with the random delays and fails.
- Possibility to use CREATE2 ChangePubKey and Transfer in a single batch.

### Fixed

## Release 2021-02-02

### Removed

- `MetricsCounter` structure was removed because it is not used.
- The limit on the number of withdrawals in the block.

### Changed

- Type aliases (`TokenId`, `AccountId`, `Nonce`, `BlockNumber`, `PriorityOpId`, `EthBlockId`) are replaced with wrapper
  structures.
- `prometheus_exporter` was made a library to be used by several crates.
- `prover_run_for_next_commit` function uses a parameterized timeout instead of a hard-coded one.
- (`storage`): `action_type` column type in db is changed from `text` to `enum` for optimization.
- (`FeeTicker`): Increased gas price estimate for transaction.
- (`loadtest`): Scenario execution was made parallel.
- Increased completeWithdrawal gas limit, that decreased the chance of the users to face the out-of-gas error

### Added

- `prometheus_exporter` is launched by every microservice.
- `tokens_acceptable_for_fees` endpoint that returns the list of tokens acceptable for fees was added to REST API v0.1.

### Fixed

- (`FeeTicker`): Performance for getting the batch fee was heavily optimized.

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
