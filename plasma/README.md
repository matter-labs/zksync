# Rollup: sidechain governed by SNARKs

Spec: https://hackmd.io/cY-VP7SDTUGgPOzDiEU3TQ

## Prerequisite

Install the latest rust version (>= 1.32):

```
rustc --version
rustc 1.32.0-nightly (21f268495 2018-12-02)
```

## Setup postgres database

### Testing

- Install postgres locally
- Install diesel-cli:

```cargo install diesel_cli --no-default-features --features postgres```

- From `server` dir run

```diesel database setup```

This will create database 'plasma' (db url is set in [server/.env] file) with our schema.

- To reset migrations, run

```diesel migration redo```

- Run test to make sure everything works:

```cargo test --lib -- --nocapture test_store_state```

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
mv -f *_pk.key ./server/
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

- copy `contracts/scripts/deploy_example.sh` to `contracts/deploy.sh`
- add mnemonics
- add infura id to `WEB3_URL` as: `WEB3_URL=https://rinkeby.infura.io/{infura_project_id}` (optional, seems to work without it too)
- launch `./deploy.sh`

Update addresses (make sure to exclude 0x !):

- copy contracts address of `PlasmaContract` to `CONTRACT_ADDR` in `server/.env` 
- in the same file, set up proper values for `SENDER_ACCOUNT` and `PRIVATE_KEY`

### Publish source

```
yarn flatten
```

## Server

Copy `server/start_demo_example.sh` by inserting proper URLs, addresses and keys

### Running locally

```shell
cd server
./run.sh
```

### Running in production

To launch and restart:

```shell
cd server
./launch.sh
```

To stop (Note, that Ctrl+C won't work! You need to run stop from a new terminal):

```shell
./stop.sh
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
./scripts/deploy-client
```
