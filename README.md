# FRANKLIN Rollup: sidechain governed by SNARKs

Spec: https://hackmd.io/cY-VP7SDTUGgPOzDiEU3TQ

# Basics

## Setup local dev environment

- Prepare dev environment prerequisites: see [docs/setup-dev.md](docs/setup-dev.md)

- Start the dev environment services:
```franklin dev-up```
- Setup env config:
```cp etc/env/dev.env.example etc/env/dev.env```
- Create `plasma` database:
```franklin db-setup```
- Deploy contracts:
```franklin deploy-contracts```

To redeploy db and contraсts:

```franklin redeploy```

To reset the dev environment:

- Stop services:
```franklin  dev-down```
- Remove mounted container data:
```rm -rf ./volumes```
- Repeat the setup procedure above

## Environments

List:
```franklin env```

Switch:
```franklin env <ENV_NAME>```

## Monitoring & management:

Seed for Metamask: fine music test violin matrix prize squirrel panther purchase material script deal
Geth: ```geth attach http://localhost:8545```

NOTE: if you are resetting geth, each Metamask account must be manually reset via Settings > Advanced > Reset account.

# Blockscout (local blockchain explorer)

- Migrate blockscout (do this once to setup database):
```make blockscout-migrate```
- Start:
```make blockscout-up```
- Stop:
```make blockscout-down```

Blockscout will be available at http://localhost:4000/txs

It generates quite some CPU load, use with caution.

## Build and run server + prover locally:

```
run-server
run-prover
run-client
```

Client UI will be available at http://localhost:8080

## Start server and prover as local docker containers:

- Start:
```make start```
- Watch logs:
```make logs```
- Stop:
```make stop```

## Build and push images to dockerhub:

```make push```

# Development

## Database migrations

```
cd src/storage
diesel database setup
```

This will create database 'plasma' (db url is set in [server/.env] file) with our schema.

- Rename `server/storage/schema.rs.generated` to `schema.rs`

- To reset migrations (will reset the db), run:

```diesel migration reset```

- Run tests:

```db-tests```

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
yarn build
```

IMPORTANT! Generated `.abi` and `.bin` files are fed to cargo to build module `plasma::eth`. 

So you need to rebuild the code on every change (to be automated soon).

### Deploy contracts

After the keys have been generated and copied to contracts:

- run `redeploy`

Update addresses (make sure to exclude 0x !):

- copy contracts address of `PlasmaContract` to `CONTRACT_ADDR` in `/env` 

### Publish source

```
yarn flatten
```

NOTE: Python >= 3.5 and pip is required for solidity flattener. You might want to run `brew upgrade python`
