# Rust SDK Changelog

All notable changes to `zksync_rs` will be documented in this file.

## Unreleased

### Added

- `PriorityOpHandle` structure, allowing awaiting for the priority operations execution.
- `PriorityOpHolder::priority_op_handle` method, allowing to get `PriorityOpHandle` out of the Ethereum transaction
  logs.
- `mint` feature with `mint_erc20` for minting ERC-20 tokens.
- `EthereumProvider::erc20_balance` method for getting the balance of ERC-20 token.

### Changed

- Hardcode gas limit for `depositERC20` for each token.

### Deprecated

### Fixed

## Version 0.3.0 (15.02.2021)

### Added

- Constructor of RpcProvider from address and network.
- Support of the new contracts upgrade functionality.

## Version 0.2.2

### Added

- Additional unit tests.
- Exporting `closest_greater_or_eq_packable_fee_amount` and `closest_greater_or_eq_packable_token_amount` functions.

### Changed

- Improved overall quality of code.
- `Wallet::is_signing_key_set` instead of checking if there is any `signing_key` at all, now checks if the `signer`'s
  public key is the same as the public key that is set in zkSync.

## Prior to 2020-12-10

**Version 0.2.0** is released.
