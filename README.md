# zkSync: scaling and privacy engine for Ethereum

[![Logo](zkSyncLogo.svg)](https://zksync.io/)

[![Live on Mainnet](https://img.shields.io/badge/wallet-Live%20on%20Mainnet-blue)](https://wallet.zksync.io)
[![Live on Rinkeby](https://img.shields.io/badge/wallet-Live%20on%20Rinkeby-blue)](https://rinkeby.zksync.io)
[![
- Thanks to validity proofs, neither users nor a single other trusted party needs to be online to monitor Rollup blocks
  in order to prevent fraud.

In other words, ZK Rollup strictly inherits the security guarantees of the underlying L1.

To learn how to use zkSync, please refer to the [zkSync SDK documentation](https://zksync.io/api/sdk/).

## Development Documentation

The following guides for developers are available:

- Installing development dependencies: [docs/setup-dev.md](docs/setup-dev.md).
- Launching zkSync locally: [docs/launch.md](docs/launch.md).
- Development guide: [docs/development.md](docs/development.md).
- Repository architecture overview: [docs/architecture.md](docs/architecture.md).

## Projects

- [zkSync server](core/bin/server)
- [zkSync prover](core/bin/prover)
- [JavaScript SDK](sdk/zksync.js)
- [Rust SDK](sdk/zksync-rs)

## Changelog

Since the repository is big and is split into independent components, there is a different changelog for each of its
major parts:

- [Smart contracts](changelog/contracts.md)
- [Core](changelog/core.md)
- [Infrastructure](changelog/infrastructure.md)
- [JavaScript SDK](changelog/js-sdk.md)
- [Rust SDK](changelog/rust-sdk.md)

## License

zkSync is distributed under the terms of both the MIT license and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.
