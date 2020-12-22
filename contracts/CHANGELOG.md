### Contracts v3 and protocol (4.09.2020)

- Change pubkey operation requires fee for processing.
- Added support for the forced exit operation which allows user to force a withdrawal from another account that does not
  have signing key set and is older than 24h.

### Contracts v2 (20.07.2020)

- Added event denoting information about pending and completed withdrawals.
  [bb0d1bd](https://github.com/matter-labs/zksync/commit/bb0d1bd)
- Added support for tokens that aren't fully compatible with ERC20.
  [c088328](https://github.com/matter-labs/zksync/commit/c088328)
- Block revert interval is changed to 0 hours. [c088328](https://github.com/matter-labs/zksync/commit/c088328)
- Redundant priority request check is removed from contract upgrade logic.
  [c088328](https://github.com/matter-labs/zksync/commit/c088328)
- `PRIORITY_EXPRIRATION_PERIOD` is reduced to 3 days. [c12ab40](https://github.com/matter-labs/zksync/commit/c12ab40)
- `UPGRADE_NOTICE_PERIOD` is increased to 8 days. [c12ab40](https://github.com/matter-labs/zksync/commit/c12ab40)
