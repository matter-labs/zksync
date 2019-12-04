# ZK Sync: scaling and privacy engine for Ethereum

Check out [ZK Sync live demo](https://demo.matter-labs.io/).

ZK Sync is a scaling and privacy engine for Ethereum. Its current functionality scope includes low gas transfers of ETH and ERC20 tokens in the Ethereum network. This document is a description of the JS library that can be used to interact with ZK Sync. 

ZK Sync is built on ZK Rollup architecture. ZK Rollup is an L2 scaling solution in which all funds are held by a smart contract on the mainchain, while computation and storage are performed off-chain. For every Rollup block, a state transition zero-knowledge proof (SNARK) is generated and verified by the mainchain contract. This SNARK includes the proof of the validity of every single transaction in the Rollup block. Additionally, the public data update for every block is published over the mainchain network in the cheap calldata.

This architecture provides the following guarantees:

- The Rollup validator(s) can never corrupt the state or steal funds (unlike Sidechains).
- Users can always retrieve the funds from the Rollup even if validator(s) stop cooperating because the data is available (unlike Plasma).
- Thanks to validity proofs, neither users nor a single other trusted party needs to be online to monitor Rollup blocks in order to prevent fraud.

In other words, ZK Rollup strictly inherits the security guarantees of the underlying L1.

To learn how to use ZK Sync, please refer to the [ZK Sync SDK documentation](https://matter-labs.io/zksync.js-docs/).

# Development

The legacy name `franklin` is still used in many places of the code.

## Prerequisites

Prepare dev environment prerequisites: see [docs/setup-dev.md](docs/setup-dev.md)

## Setup local dev environment

Setup:

```
franklin dev-up
franklin init
```

To completely reset the dev environment:

- Stop services:
```franklin dev-down```
- Remove containers data:
```
ssh minikube
rm -r /data/*
```
- Repeat the setup procedure above

# (Re)deploy db and contraсts:

```franklin redeploy```

## Environment configurations

Env config files are held in `etc/env/`

List configurations:

```franklin env```

Switch between configurations:

```franklin env <ENV_NAME>```

## Monitoring & management:

Seed for Metamask: fine music test violin matrix prize squirrel panther purchase material script deal
Geth: ```geth attach $(bin/kube-geth-url)```

NOTE: if you are resetting geth, each Metamask account must be manually reset via Settings > Advanced > Reset account.

## Build and run server + prover locally for development:

Run server:
```
franklin server
```

By default block chunk size set to `100`. For testing & development purposes you
ca change it to smaller values. Two places requires a change:
1. Environment variable value in `./etc/env/dev.env` `BLOCK_SIZE_CHUNKS`
2. Rust constant at `./core/models/params.rs` `BLOCK_SIZE_CHUNKS`
If you apply changes, do not forget to redeploy contracts `franklin redeploy`.

You must prepare keys. This only needs to be done once:
```
./bin/gen-keys
franklin redeploy
```
Run prover:
```
franklin prover
```

Run client
```
franklin client
```

Client UI will be available at http://localhost:8080.
Make sure you have environment variables set right, you can check it by running:
```franklin env```. You should see `* dev` in output.

## Start server and prover in minikube (this setup is closest to prod):

- Prerequisite: ```franklin dev-up; franklin init```

- Start:
```franklin start```

- Watch logs:
Server: ```franklin log-server```
Prover: ```franklin log-prover```

- Stop:
```franklin stop```

## Build and push images to dockerhub:

```franklin dockerhub-push```

# Development

## Database migrations

- ```cd core/storage```
- Add diesel migration
- Rename `core/storage/schema.rs.generated` to `schema.rs`
- Run tests: ```franklin db-tests```

## Generating keys

To generate a proving key, from `server` dir run:

```
cargo run --release --bin read_write_keys
```

It will generate a `*VerificationKey.sol` and `*_pk.key` files for 'deposit', 'exit' and 'transfer' circuits in the root folder.

Move files to proper locations:

```shell
mv -f n*VerificationKey.sol ./contracts/contracts/
mv -f *_pk.key ./prover/keys/
```

If the pregenerated leaf format changes, replace the `EMPTY_TREE_ROOT` constant in `contracts/contracts/PlasmaStorage.sol`.

## Contracts

### Re-build contracts:

```
cd contracts; yarn build
```

IMPORTANT! Generated `.abi` and `.bin` files are fed to cargo to build module `plasma::eth`. 

So you need to rebuild the code on every change (to be automated).

### Publish source code on etherscan

```
franklin publish-source
```

# License

ZK Sync is distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.