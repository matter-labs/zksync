# Core Components Changelog
All notable changes to the core components will be documented in this file.

## Unrealesed

## Prior to 2020-12-23

- Robustness of the fee ticker's API interacting module was increased.
- A possibility to get an Ethereum tx hash for withdrawal operation was added.
- Bug with delay between receiving a job and starting sending heartbeats was fixed.
- Blocks that contain withdraw operations are sealed faster.
- Added support for non-standard Ethereum signatures.
- `eth_sender` module now can be disabled. 
- Transfer to zero address (0x00..00) is now forbidden in zkSync.
- WebSocket server now uses more threads for handling incoming requests.
