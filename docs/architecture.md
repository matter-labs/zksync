# zkSync project architecture

This document covers the structure of this repository.

## High-Level Overview

zkSync repository consists of several applications:

- zkSync smart contract: a Solidity smart contract deployed on the Ethereum blockchain,
  which manages users' balances and verifies the correctness of operations performed within
  zkSync network.
- Prover application: a worker application that creates a proof for an executed block.
  Prover applications poll Server application for available jobs, and once there is a new
  block, server provides a witness (input data to generate a proof), and prover starts working.
  Once proof is generated, it is reported to the Server application, and Server publishes the
  proof to the smart contract.
  Prover application is considered an on-demand worker, thus it is OK to have many provers (if
  server load is high) or no provers at all (if there are no incoming transactions).
  Generating a proof is a very resource consuming work, thus machines that run a prover application
  must have a modern CPU and a lot of RAM.
- Server application: a node running the zkSync network. It is capable of following things:
  - Monitoring the smart contract for the onchain operations (such as deposits).
  - Accepting transactions.
  - Generating zkSync chain blocks.
  - Requesting proofs for executed blocks.
  - Publishing data to the smart contract.

  Server application exists in two available forms:

  - Monolithic application, which provides all the required functionality from one binary.
    This form is convenient for the development needs.
    Corresponding crate is `core/bin/server`.
  - Microservices applications, which are capable of working independently from each other:
    - `Core` service (`core/bin/zksync_core`) maintains transactions memory pool and commits new blocks.
    - `API` service (`core/bin/zksync_api`) provides a server "front-end": REST API & JSON RPC HTTP/WS implementations.
    - `Ethereum Sender` service (`core/bin/zksync_eth_sender`) finalizes the blocks by sending corresponding Ethereum transactions to the
      L1 smart contract.
    - `Witness Generator` service (`core/bin/zksync_witness_generator`) creates input data required for provers to prove blocks, and
      implements a private API server for provers to interact with.
    - `Prometheus Exporter` service (`core/bin/zksync_prometheus_exporter`) manages exporting data about the application state
      for further node behavior analysis.
- Explorer: zkSync network explorer. A web application that receives data from the Server API
  and renders it to the convenient blockchain explorer interface.
  
Thus, in order to get a local zkSync setup running, the following has to be done:

- zkSync smart contract is compiled and deployed to the Ethereum.
- zkSync server is launched.
- At least one prover is launched and connected to the Server application.


## Low-Level Overview

This section provides an overview on folders / sub-projects that exist in this repository.

- `/bin`: Infrastructure scripts which help to work with zkSync applications.
- `/contracts`: Everything related to zkSync smart-contract.
  - `/contracts`: Smart contracts code
  - `/scripts` && `/src.ts`: TypeScript scripts for smart contracts management.
- `/core`: Code of the sub-projects that implement zkSync network.
  - `/bin`: Applications mandatory for zkSync network to operate.
    - `/server`: zkSync server application.
    - `/prover`: zkSync prover application.
    - `/data_restore`: Utility to restore a state of the zkSync network from a smart contract.
    - `/key_generator`: Utility to generate verification keys for network.
    - `/gen_token_add_contract`: Utility to generate `TokenDeployInit` smart contract, required for initial network launch.
    - `/parse_pub_data`: Utility to parse zkSync operation pubdata.
    - `/zksync_core`: zkSync server Core microservice.
    - `/zksync_api`: zkSync server API microservice.
    - `/zksync_eth_sender`: zkSync server Ethereum sender microservice.
    - `/zksync_witness_generator`: zkSync server Witness Generator & Prover Server microservice.
    - `/zksync_prometheus_exporter`: zkSync server Prometheus data exporter microservice.
  - `/lib`: Dependencies of the binaries above.
    - `/basic_types`: Crate with declaration of the essential zkSync primitives, such as `address`.
    - `/circuit`: Cryptographic environment enforsing the correctness of executed transactions in the zkSync network.
    - `/config`: Utilities to load configuration options of zkSync applications.
    - `/contracts`: Loaders for zkSync contracts interfaces and ABI.
    - `/crypto`: Cryptographical primitives using among zkSync crates.
    - `/eth_client`: Module providing an interface to interact with an Ethereum node.
    - `/prover_utils`: Utilities related to the proof generation.
    - `/state`: A fast pre-circuit executor for zkSync transactions used on the Server level to generate blocks.
    - `/storage`: An encapsulated database interface.
    - `/types`: zkSync network operations, transactions and common types.
    - `/utils`: Miscellaneous helpers for zkSync crates.
    - `/vlog`: An utility library for verbose logging.
  - `/tests`: Testing infrastructure for zkSync network.
    - `/loadtest`: An application for highload testing of zkSync server.
    - `/test_account`: A representation of zkSync account which can be used for tests.
    - `/testkit`: A relatively low-level testing library and test suite for zkSync.
    - `/ts-test`: Integration tests set implemented in TypeScript. Requires a running Server and Prover applications to operate.
- `/docker`: Dockerfiles used for development of zkSync and for packaging zkSync for a production environment.
- `/etc`: Configration files.
  - `/env`: `.env` files that contain environment variables for different configuration of zkSync Server / Prover.
  - `/js`: Configuration files for JavaScript applications (such as Explorer).
  - `/tesseracts`: Configuration for `tesseracts` minimalistic blockchain explorer (used for development).
  - `/tokens`: Configuration of supported Ethereum ERC-20 tokens.
- `/infrastructure`: Application that aren't naturally a part of zkSync core, but are related to it.
  - `/analytics`: Script that analyzes the costs of zkSync network maintaining.
  - `/explorer`: A blockchain explorer for zkSync network.
  - `/fee-seller`: Script to sell the collected fees.
  - `/tok_cli`: A command-line utility for adding new supported tokens into zkSync
  - `/zcli`: A command-line interface and development wallet for zkSync network.
- `/keys`: Verification keys for `circuit` module.
- `/sdk`: Implementation of client libraries for zkSync network in different programming languages.
  - `/zksync-crypto`: zkSync network cryptographic primitives, which can be compiled to WASM.
  - `/zksync.js`: A JavaScript / TypeScript client library for zkSync.
  - `/zksync-rs`: Rust client library for zkSync.
