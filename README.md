# FRANKLIN Rollup: sidechain governed by SNARKs

Spec: https://hackmd.io/cY-VP7SDTUGgPOzDiEU3TQ

# Basics

## Prerequisites

Prepare dev environment prerequisites: see [docs/setup-dev.md](docs/setup-dev.md)

## Setup local dev environment

First-time setup:

```franklin init```

To completely reset the dev environment:

- Stop services:
```franklin dev-down```
- Remove mounted container data:
```rm -rf ./volumes```
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
Geth: ```geth attach http://localhost:8545```

NOTE: if you are resetting geth, each Metamask account must be manually reset via Settings > Advanced > Reset account.

# Blockscout (local blockchain explorer)

It generates quite some CPU load, but might be useful to visualize blockchain activity. Use with caution.

- Migrate blockscout (do this once to setup database):
```franklin blockscout-migrate```

- Start:
```franklin blockscout-up```

- Stop:
```franklin blockscout-down```

Blockscout will be available at http://localhost:4000/txs

## Build and run server + prover locally:

```
franklin server
franklin prover
franklin client
```

Client UI will be available at http://localhost:8080

## Start server and prover as local docker containers:

- Start:
```franklin start```

- Watch logs:
```franklin logs```

- Stop:
```franklin stop```

## Build and push images to dockerhub:

```franklin push```

# Development

## Database migrations

- ```cd src/storage```
- Add diesel migration
- Rename `src/storage/schema.rs.generated` to `schema.rs`
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

## Contratcs

### Re-build contracts:

```
cd contracts; yarn build
```

IMPORTANT! Generated `.abi` and `.bin` files are fed to cargo to build module `plasma::eth`. 

So you need to rebuild the code on every change (to be automated).

### Publish source code on etherscan

```
franklin flatten source
```