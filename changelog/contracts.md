# Smart Contracts' Changelog

All notable changes to the contracts will be documented in this file.

## 2020-09-04

**Version 3** is released.

### Added

- `ForcedExit` operation which allows user to force a withdrawal from another account that does not have signing key set
  and is older than 24h.

### Changed

- `ChangePubKey` operation requires fee for processing.

## 2020-07-20

**Version 2** is released.

### Added

- Event denoting information about pending and completed withdrawals.
- Support for tokens that aren't fully compatible with ERC20.

### Changed

- Block revert interval is 0 hours.
- `PRIORITY_EXPRIRATION_PERIOD` is reduced to 3 days.
- `UPGRADE_NOTICE_PERIOD` is increased to 8 days.

### Removed

- Redundant priority request check is removed from contract upgrade logic.
