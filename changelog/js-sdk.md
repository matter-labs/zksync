# JavaScript SDK changelog

All notable changes to `zksync.js` will be documented in this file.

## Version 0.11.0

### Added

- Methods for working with NFTs. You can read more [here](https://zksync.io/dev/nfts.html).
- Methods for working with atomic swaps/limit orders. You can read more [here](https://zksync.io/dev/swaps.html).

### Changed

- `zksync-crypto` to support atomic swaps/limit orders functionality.

## Version 0.10.9 (13.04.2021)

### Changed

- Exported classes: `ETHOperation`, `Transaction`, `ZKSyncTxError`
- Exported types: `TotalFee`

## Version 0.10.6 (16.03.2021)

### Added

- (`BatchBuilder`) Make it possible to add signed `ChangePubKey` transaction to the batch.

## Version 0.10.4 (08.03.2021)

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

## Version 0.9.0 (15.02.2021)

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
