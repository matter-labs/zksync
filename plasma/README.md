# Rollup: sidechain governed by SNARKs

Spec: https://hackmd.io/cY-VP7SDTUGgPOzDiEU3TQ

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

## Contratcs

### Install truffle and dependencies:

```
cd contracts
yarn
```

### Re-build contracts:

```
yarn build
```

IMPORTANT! Generated `.abi` and `.bin` files are fed to cargo to build module `plasma::eth`. 

So you need to rebuild the code on every change (to be automated soon).

### Deploy contracts

After the keys have been generated and copied to contracts:

- copy `contracts/scripts/deploy_example.sh` to `contracts/deploy.sh`
- add mnemonics and infura id
- launch `./deploy.sh`

Update addresses (make sure to exclude 0x !):

- copy contracts address of `PlasmaContract` to `CONTRACT_ADDR` in `server/.env` 
- in the same file, set up proper values for `SENDER_ACCOUNT` and `PRIVATE_KEY`

See [contracts/README.md] for more options.

## Prepare server scripts

Copy `server/start_demo_example.sh` by inserting proper URLs, addresses and keys

## Running locally

```shell
cd server
./run.sh
```

## Running as server

To launch and restart:

```shell
cd server
./launch.sh
```

To stop:

```shell
./stop.sh
```

## Deploy client

```
./scripts/deploy-client
```
