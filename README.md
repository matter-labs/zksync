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

During first init you have to download around 8 GB of setup files, this should be done once.
If you have problem on this step of init see help of the `zksync plonk-setup`.

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
Server is configured using env files in `./etc/env` directory. 
After first init you should have `./etc/env/dev.env` file copied from `./etc/env/dev.env.example`.

Server can produce block of different sizes, all available sizes are in the `SUPPORTED_BLOCK_CHUNKS_SIZES` env variable.
You can select wich of these block sizes are produced by you server using `BLOCK_CHUNK_SIZES` env variable.

Note: for proof generation for large blocks you need a lot of resources and on average user machine 
you should be able to proof only the smallest of the available block sizes.

After that you may need to invalidate `cargo` cache by touching the files of `models`:

```sh
touch core/models/**/*.rs
```

This is required, because `models` take the environment variable value at the compile time, and
we have to recompile this module to set correct values.

If you use additional caching systems (like `sccache`), you may have to remove their cache as well.

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

## Testing

- Running all the `rust` tests:
  
  ```sh
  f cargo test
  ```

- Running the database tests:
  
  ```sh
  zksync db-tests
  ```
- Running the integration test:
  
  ```sh
  zksync server # Has to be run in the 1st terminal
  zksync prover # Has to be run in the 2nd terminal
  zksync integration-simple # Has to be run in the 3rd terminal
  ```

- Running the full integration tests (similar to `integration-simple`, but performs different full exits)
  
  ```sh
  zksync server # Has to be run in the 1st terminal
  zksync prover # Has to be run in the 2nd terminal
  zksync integration-full-exit # Has to be run in the 3rd terminal
  ```

- Running the circuit tests:
  
  ```sh
  zksync circuit-tests
  ```

- Running the prover tests:
  
  ```sh
  zksync prover-tests
  ```

- Running the benchmarks:
  
  ```sh
  f cargo bench
  ```

- Running  the loadtest:

  ```sh
  zksync server # Has to be run in the 1st terminal
  zksync prover # Has to be run in the 2nd terminal
  zksync loadtest # Has to be run in the 3rd terminal
  ```

## Using Dummy Prover

Using the real prover for the development can be not really handy, since it's pretty slow and resource consuming.

Instead, one may want to use the Dummy Prover: lightweight version of prover, which does not actually proves anything,
but acts like it does.

To enable the dummy prover, run:

```sh
zksync dummy-prover enable
```

And after that you will be able to use the dummy prover instead of actual prover:

```sh
zksync dummy-prover # Instead of `zksync prover`
```

**Warning:** `setup-dummy-prover` subcommand changes the `Verifier.sol` contract, which is a part of `git` repository.
Be sure not to commit these changes when using the dummy prover!

If one will need to switch back to the real prover, a following command is required:

```sh
zksync dummy-prover disable
```

This command will revert changes in the contract and redeploy it, so the actual prover will be usable again.

Also you can always check the current status of the dummy verifier:

```sh
$ zksync dummy-prover status
Dummy Verifier status: disabled
```


## Developing circuit

* To generate proofs you need universal setup files that you downloaded during first init. 
* To verify generated proofs you need verification keys for generated for specific circuit and Verifier.sol contract to check proofs on the Ethereum network.

Steps to do after updating circuit:
1. Update circuit version by updating `KEY_DIR` in your env file (don't forget to place it to `dev.env.example`)
(last parts of this variable usually means last commit where you updated circuit).
2. Regenerate verification keys and Verifier contract using `zksync verify-keys gen` command.
3. Pack generated verification keys using `zksync verify-keys pack` command and commit resulting file to repo.


## Contracts

### Re-build contracts:

```sh
zksync build-contracts
```

### Publish source code on etherscan

```sh
zksync publish-source
```

# License

ZK Sync is distributed under the terms of both the MIT license
and the Apache License (Version 2.0).

See [LICENSE-APACHE](LICENSE-APACHE), [LICENSE-MIT](LICENSE-MIT) for details.
