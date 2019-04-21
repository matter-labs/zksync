# FRANKLIN Rollup: sidechain governed by SNARKs

Spec: https://hackmd.io/cY-VP7SDTUGgPOzDiEU3TQ

## Setup local dev environment

- Install prerequisites: see [docs/setup-dev.md](docs/setup-dev.md)
- Add `./bin` to `PATH`

- Migrate blockscout (do this before starting `make dev-up`):
```make migrate-blockscout```

- Start the dev environment:
```make dev-up```
- Create `plasma` database:
```db-setup```
- Deploy contracts:
```deploy-contracts``

## Management:

Seed for Metamask: fine music test violin matrix prize squirrel panther purchase material script deal
Geth: ```geth attach http://localhost:8545```
Blockscout explorer: http://localhost:4000/txs

## Build and run server + prover locally:

```
run-server
run-prover
```

## Server and prover as local docker containers:

```
make up
make logs
make down
```

## Build and push images to dockerhub:

```
make push
```

---

# Details

## Local geth

1. Follow the instruction here: https://hackernoon.com/hands-on-creating-your-own-local-private-geth-node-beginner-friendly-3d45902cc612
2. However, set the gaslimit to 8M *before* starting the geth for the first time!

## Config

All environment variables must be located in a single file `/env`.

- Copy `/env.example` to `/env` and set all of them correctly

## Database migrations

```
cd src/storage
diesel database setup
```

This will create database 'plasma' (db url is set in [server/.env] file) with our schema.

- Rename `server/storage/schema.rs.generated` to `schema.rs`

- To reset migrations (will reset the db), run:

```diesel migration redo```

- Run tests:

```db-tests```

### Production

For production, `DATABSE_URL` env var must be set properly.

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

## Web3 provider

In the `server/.env` set up `CHAIN_ID` and `WEB3_URL` accordingly.

## Contratcs

### Install truffle and dependencies:

```
cd contracts
yarn
```

NOTE: Python >= 3.5 and pip is required for solidity flattener. You might want to run `brew upgrade python`

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

## Server

### Running locally

```shell
run
```

### Running in production

To launch and restart:

```shell
launch
```

To stop (Note, that Ctrl+C won't work! You need to run stop from a new terminal):

```shell
stop
```

## Client UI

### Run locally

``` bash
# install dependencies
yarn

# serve with hot reload at localhost:8080; API server will be queried at localhost:3000
yarn run dev

# build for production with minification
yarn run build
```

### Deploy client publicly

Single command to build and deploy to github pages:

```
update-client
```
