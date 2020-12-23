# Smart Contracts' Changelog

All notable changes to the contracts will be documented in this file.

## Unreleased

## 2020-09-04

**Version 3** is released.

- Change pubkey operation requires fee for processing.
- Added support for the forced exit operation which allows user to force a withdrawal from another account that does not
  have signing key set and is older than 24h.

## 2020-07-20

**Version 2** is released.

- Added event denoting information about pending and completed withdrawals.
- Added support for tokens that aren't fully compatible with ERC20.
- Block revert interval is changed to 0 hours.
- Redundant priority request check is removed from contract upgrade logic.
- `PRIORITY_EXPRIRATION_PERIOD` is reduced to 3 days.
- `UPGRADE_NOTICE_PERIOD` is increased to 8 days.
