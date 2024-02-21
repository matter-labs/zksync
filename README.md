# zkSync: scaling and privacy engine for Ethereum

[![Logo](zkSyncLogo.svg)](https://zksync.io/)

[![Live on Mainnet](https://img.shields.io/badge/wallet-Live%20on%20Mainnet-blue)](https://wallet.zksync.io)
[![Live on Rinkeby](https://img.shields.io/badge/wallet-Live%20on%20Rinkeby-blue)](https://rinkeby.zksync.io)
[![Live on Ropsten](https://img.shields.io/badge/wallet-Live%20on%20Ropsten-blue)](https://ropsten.zksync.io)

zkSync is a scaling and privacy engine for Ethereum. Its current functionality scope includes low gas transfers of ETH
and ERC20 tokens in the Ethereum network.

## Description

zkSync is built on ZK Rollup architecture. ZK Rollup is an L2 scaling solution in which all funds are held by a smart
contract on the mainchain, while computation and storage are performed off-chain. For every Rollup block, a state
transition zero-knowledge proof (SNARK) is generated and verified by the mainchain contract. This SNARK includes the
proof of the validity of every single transaction in the Rollup block. Additionally, the public data update for every
block is published over the mainchain network in the cheap calldata.

This architecture provides the following guarantees:

- The Rollup validator(s) can never corrupt the state or steal funds (unlike Sidechains).
- Users can always retrieve the funds from the Rollup even if validator(s) stop cooperating because the data is
  available (unlike Plasma).
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

## Project scripts 

### build:zksync-sdk
This script is used to build the zksync-sdk package within the monorepo. It executes the yarn zksync prepublish command, which likely triggers the build process for the zksync-sdk package.

### build:crypto
This script is responsible for building the crypto package within the monorepo. It executes the yarn crypto build command, which presumably initiates the build process for the crypto package.

### build:reading-tool
This script facilitates the build process for the reading-tool package within the monorepo. It executes the yarn reading-tool build command, which likely triggers the build process for the reading-tool package.

### zksync
This script provides a shortcut to execute commands specifically within the zksync workspace. It likely simplifies the process of running commands related to the zksync package.

### crypto
This script serves as a shortcut to execute commands within the zksync-crypto workspace. It simplifies the process of running commands related to the crypto package.

### contracts
This script acts as a shortcut to execute commands within the franklin-contracts workspace. It simplifies the process of running commands related to the contracts package.

### ts-tests
This script provides a convenient way to execute commands within the ts-tests workspace. It simplifies the process of running commands related to testing TypeScript code.

### zk
This script acts as a shortcut to execute commands within the zk workspace. It simplifies the process of running commands related to the zk package.

### reading-tool
This script serves as a shortcut to execute commands within the reading-tool workspace. It simplifies the process of running commands related to the reading-tool package.

### api-docs
This script provides a convenient way to execute commands within the api-docs workspace. It simplifies the process of generating or managing API documentation within the monorepo.

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
