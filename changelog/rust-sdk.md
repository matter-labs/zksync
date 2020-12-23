# Rust SDK Changelog

All notable changes to `zksync_rs` will be documented in this file.

## Unrealesed

**Version 0.2.2** is being developed.

### Added

- Additional Unit tests.

### Changed

- Improved overall quality of code.
- `Wallet::is_signing_key_set` instead of checking if there is any `signing_key` at all, now checks if the `signer`'s
  public key is the same as the public key that is set in zkSync.

## Prior to 2020-12-10

**Version 0.2.0** is released.
