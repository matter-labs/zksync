# JavaScript SDK changelog

All notable changes to `zksync.js` will be documented in this file.

## Version 0.12.0

! Important, version 0.12.0 contains breaking changes, please make fixes before upgrading this npm package.

### Added

- We've added `remote json rpc signer` which means you could add support of zkSync L2 Wallets such as Argent zkSync or
  other applications into your dapp. Read more here
  <http://docs.zksync.io/api/sdk/js/accounts.html#creating-wallet-from-l2-wallets>

### Changed

- `getOrder` renamed to `signOrder`. The method was used for signing and the name of the method was inconsistent.
- `getLimitOrder` was renamed to `signLimitOrder`. The method was used for signing and the name of the method was
  inconsistent.
- All methods whose name started with `get` for example `getTransfer` were deleted. Now for this purpose, you could use
  `BatchBuilder.`

### Deprecated

### Fixed

## Version 0.11.0

### Added

- Methods for working with NFTs. You can read more [here](https://zksync.io/dev/nfts.html).
- Methods for working with atomic swaps/limit orders. You can read more [here](https://zksync.io/dev/swaps.html).
- `RestProvider` class, that is used for querying REST API v0.2.
- `SyncProvider` interface: common interface for API v0.2 `RestProvider` and JSON RPC `Provider`.
- Types for REST API v0.2.

- `RestProvider` class, that is used for querying REST API v0.2.
- `SyncProvider` interface: common interface for API v0.2 `RestProvider` and JSON RPC `Provider`.
- Types for REST API v0.2.

### Changed

- Changed type of `provider` field in `Wallet` class from `Provider` to `SyncProvider`.
- `ForcedExit` fee type is used for `ForcedExit` transactions instead of `Withdraw` fee type.
- `zksync-crypto` to support atomic swaps/limit orders functionality.
- Changed type of `provider` field in `Wallet` class from `Provider` to `SyncProvider`.
- `ForcedExit` fee type is used for `ForcedExit` transactions instead of `Withdraw` fee type.

### Deprecated

### Fixed

## Version 0.10.9 (2021-04-13)

### Changed

- Exported classes: `ETHOperation`, `Transaction`, `ZKSyncTxError`
- Exported types: `TotalFee`

## Version 0.10.6 (2021-03-16)

### Added

- (`BatchBuilder`) Make it possible to add signed `ChangePubKey` transaction to the batch.

## Version 0.10.4 (2021-03-08)

### Added

- Method for calculation of transaction hash.
- Support for environments without WebAssembly.

### Changed

- Hardcode gas limit for `depositERC20` for each token.

### Deprecated

- `Signer.transferSignBytes` method
- `Signer.withdrawSignBytes` method
- `Signer.forcedExitSignBytes` method
- `Signer.changePubKeySignBytes` method

### Fixed

## Version 0.9.0 (2021-02-15)

### Added

- Support of the new contracts upgrade functionality.
- BatchBuilder class for convenient batches creating.
- `zksync-crypto` release 0.4.5.

### Changed

### Deprecated

- WebSocket provider.

### Fixed

## Version 0.8.4

### Added

- `updateTokenSet` function that updates the `tokenSet` stored in the `Provider`.
- `newMockProvider` and `DummyTransport`. Tests for the library.
- `closestGreaterOrEqPackableTransactionAmount` and `closestGreaterOrEqPackableTransactionFee` functions. Tests for
  them.
- Checks for ERC-1271 wallets for whether the messages should be prefixed

### Changed

- HTTP provider is now the default one.

### Fixed

- Bug with `integerToFloat` function. Now, it really rounds to the closest less or equal float number.

## Prior to 2020-12-10

**Version 0.8.3** is released.
