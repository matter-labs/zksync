# FRANKLIN Rollup: sidechain governed by SNARKs

Spec: https://hackmd.io/cY-VP7SDTUGgPOzDiEU3TQ

# Basics

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

## Contratcs

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
