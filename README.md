# ZK Sync: scaling and privacy engine for Ethereum

Check out [ZK Sync live demo](https://demo.matter-labs.io/).

ZK Sync is a scaling and privacy engine for Ethereum. Its current functionality scope includes low gas transfers of ETH and ERC20 tokens in the Ethereum network. This document is a description of the JS library that can be used to interact with ZK Sync. 

ZK Sync is built on ZK Rollup architecture. ZK Rollup is an L2 scaling solution in which all funds are held by a smart contract on the mainchain, while computation and storage are performed off-chain. For every Rollup block, a state transition zero-knowledge proof (SNARK) is generated and verified by the mainchain contract. This SNARK includes the proof of the validity of every single transaction in the Rollup block. Additionally, the public data update for every block is published over the mainchain network in the cheap calldata.

This architecture provides the following guarantees:

- The Rollup validator(s) can never corrupt the state or steal funds (unlike Sidechains).
- Users can always retrieve the funds from the Rollup even if validator(s) stop cooperating because the data is available (unlike Plasma).
- Thanks to validity proofs, neither users nor a single other trusted party needs to be online to monitor Rollup blocks in order to prevent fraud.

In other words, ZK Rollup strictly inherits the security guarantees of the underlying L1.

To learn how to use ZK Sync, please refer to the [ZK Sync SDK documentation](https://zksync.io).

## Prerequisites

Prepare dev environment prerequisites: see [docs/setup-dev.md](docs/setup-dev.md)

## Setup local dev environment

Setup:

```sh
zksync init
```

To completely reset the dev environment:

- Stop services:
  ```sh
  zksync dev-down
  ```
- Repeat the setup procedure above

# (Re)deploy db and contraсts:

```sh
zksync redeploy
```

## Environment configurations

Env config files are held in `etc/env/`

List configurations:

```sh
zksync env
```

Switch between configurations:

```sh
zksync env <ENV_NAME>
```

## Build and run server + prover locally for development:

Run server:

```sh
zksync server
```

By default block chunk size set to `50`. For testing & development purposes you
can change it to the smaller value.

**Note:** Currently it's not recommended though. Lowering the block chunk size may
break several tests, since some of them create big blocks.

If you have to change the block chunk size anyway, you should change the environment
variable `BLOCK_SIZE_CHUNKS` value in `./etc/env/dev.env`.

After that you may need to invalidate `cargo` cache by touching the files of `models`:

```sh
touch core/models/**/*.rs
```

This is required, because `models` take the environment variable value at the compile time, and
we have to recompile this module to set correct values.

If you use additional caching systems (like `sccache`), you may have to remove their cache as well.

After that you must generate keys. This only needs to be done once:

```sh
./bin/gen-keys
zksync redeploy
```

Run prover:

```sh
zksync prover
```

Run client

```sh
zksync client
```

Client UI will be available at http://localhost:8080.
Make sure you have environment variables set right, you can check it by running:
`zksync env`. You should see `* dev` in output.

## Build and push images to dockerhub:

```sh
zksync dockerhub-push
```

# Development

## Database migrations

- 
  ```sh
  cd core/storage
  ```
- Add diesel migration
- Rename `core/storage/schema.rs.generated` to `schema.rs`
- Run tests:
  ```sh
  zksync db-tests
  ```

## Generating keys

To generate a proving key, from `server` dir run:

```sh
cargo run --release --bin read_write_keys
```

It will generate a `*VerificationKey.sol` and `*_pk.key` files for 'deposit', 'exit' and 'transfer' circuits in the root folder.

Move files to proper locations:

```sh
mv -f n*VerificationKey.sol ./contracts/contracts/
mv -f *_pk.key ./prover/keys/
```

If the pregenerated leaf format changes, replace the `EMPTY_TREE_ROOT` constant in `contracts/contracts/PlasmaStorage.sol`.

## Contracts

### Re-build contracts:

```sh
cd contracts; yarn build
```

IMPORTANT! Generated `.abi` and `.bin` files are fed to cargo to build module `plasma::eth`. 

So you need to rebuild the code on every change (to be automated).

### Publish source code on etherscan

```sh
zksync publish-source
```

# License

ZK Sync is distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.
