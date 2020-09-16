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
  - `/lib`: Dependencies of the binaries above.
    - `/circuit`: Cryptographic environment enforsing the correctness of executed transactions in the zkSync network.
    - `/crypto_exports`: Re-exports for used external cryptographic libraries.
    - `/eth_client`: Module providing an interface to interact with an Ethereum node.
    - `/models`: Various types declarations and primitive functions using throughout zkSync crates.
    - `/plasma`: A fast pre-circuit executor for zkSync transactions used on the Server level to generate blocks.
    - `/storage`: An encapsulated database interface.
    - `/vlog`: An utility library for verbose logging.
  - `/tests`: Testing infrastructure for zkSync network.
    - `/loadtest`: An application for highload testing of zkSync server.
    - `/testkit`: A relatively low-level testing library and test suite for zkSync.
    - `/ts-test`: Integration tests set implemented in TypeScript. Requires a running Server and Prover applications to operate.
- `/docker`: Dockerfiles used for development of zkSync and for packaging zkSync for a production environment.
- `/etc`: Configration files.
  - `/env`: `.env` files that contain environment variables for different configuration of zkSync Server / Prover.
  - `/js`: Configuration files for JavaScript applications (such as Explorer).
  - `/tesseracts`: Configuration for `tesseracts` minimalistic blockchain explorer (used for development).
  - `/tokens`: Configuration of supported Ethereum ERC-20 tokens.
- `/infrastructure`: Application that aren't naturally a part of zkSync core, but are related to it.
  - `/explorer`: A blockchain explorer for zkSync network.
  - `/tok_cli`: A command-line utility for adding new supported tokens into zkSync
- `/keys`: Verification keys for `circuit` module.
- `/sdk`: Implementation of client libraries for zkSync network in different programming languages.
  - `/zksync-crypto`: zkSync network cryptographic primitives, which can be compiled to WASM.
  - `/zksync.js`: A JavaScript / TypeScript client library for zkSync.
