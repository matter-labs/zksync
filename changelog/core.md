# Core Components Changelog

All notable changes to the core components will be documented in this file.

## Prior to 2020-12-25

### Added

- A possibility to get an Ethereum tx hash for withdrawal operation.
- Support for non-standard Ethereum signatures.
- Loadtest scenario for stressing transaction batches

### Changed

- Robustness of the fee ticker's API interacting module was increased.
- Blocks that contain withdraw operations are sealed faster.
- `eth_sender` module now can be disabled.
- Transfer to zero address (0x00..00) is now forbidden in zkSync.
- WebSocket server now uses more threads for handling incoming requests.

### Fixed

- Bug with delay between receiving a job and starting sending heartbeats.
