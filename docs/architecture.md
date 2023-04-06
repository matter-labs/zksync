# zkSync project architecture 项目架构

This document covers the structure of this repository.
本文介绍了zkSync代码库的结构。

## High-Level Overview 总体概述

zkSync repository consists of several applications:
zkSync代码库由以下几个应用程序组成：

- zkSync smart contract: a Solidity smart contract deployed on the Ethereum blockchain, which manages users' balances and verifies the correctness of operations performed within zkSync network.
- zkSync智能合约：这是一个在以太坊区块链上部署的Solidity智能合约，用于管理用户余额并验证在zkSync网络中执行的操作的正确性。
- Prover application: a worker application that creates a proof for an executed block. Prover applications poll Server application for available jobs, and once there is a new block, server provides a witness (input data to generate a proof), and prover starts working. Once proof is generated, it is reported to the Server application, and Server publishes the proof to the smart contract. Prover application is considered an on-demand worker, thus it is OK to have many provers (if server load is high) or no provers at all (if there are no incoming transactions). Generating a proof is a very resource consuming work, thus machines that run a prover application must have a modern CPU and a lot of RAM.
- Prover程序：这是一个工作应用程序，为执行的区块创建证明。Prover应用程序轮询Server应用程序以获取可用的作业，一旦有新的区块，服务器提供一个见证（生成证明所需的输入数据），Prover就开始工作。一旦生成了证明，它就会被报告给Server应用程序，并由Server将证明发布到智能合约中。证明生成是一个非常耗费资源的工作，因此运行证明生成器应用程序的机器必须具备现代的CPU和大量的RAM
- Server application: a node running the zkSync network. It is capable of following things:
- Server应用程序：运行zkSync网络的节点。它能够执行以下操作：
  - Monitoring the smart contract for the onchain operations (such as deposits).
  - 监听智能合约以进行链上操作（如存款）。
  - Accepting transactions.
  - 接受交易。
  - Generating zkSync chain blocks.
  - 生成zkSync链块。
  - Requesting proofs for executed blocks.
  - 请求执行块的证明。
  - Publishing data to the smart contract.
  - 将数据发布到智能合约。

  Server application exists in two available forms:
  Server应用程序有两种可用形式：

  - Monolithic application, which provides all the required functionality from one binary. This form is convenient for the development needs. Corresponding crate is `core/bin/server`.
  - 单体应用程序，从一个二进制文件提供所有必需的功能。这种形式对于开发需要非常方便。相应的crate是core/bin/server。
  - Microservices applications, which are capable of working independently from each other:
  - 微服务应用程序，能够独立于彼此工作：
    - `Core` service (`core/bin/zksync_core`) maintains transactions memory pool and commits new blocks.
    - `Core`服务（`core/bin/zksync_core`）维护交易内存池并提交新块。
    - `API` service (`core/bin/zksync_api`) provides a server "front-end": REST API & JSON RPC HTTP/WS implementations.
    - `API`服务（`core/bin/zksync_api`）提供服务器“前端”：REST API＆JSON RPC HTTP/WS实现。
    - `Ethereum Sender` service (`core/bin/zksync_eth_sender`) finalizes the blocks by sending corresponding Ethereum transactions to the L1 smart contract.
    - `Ethereum Sender`服务（`core/bin/zksync_eth_sender`）通过向L1智能合约发送相应的以太坊交易来完成块的最终化。
    - `Witness Generator` service (`core/bin/zksync_witness_generator`) creates input data required for provers to prove blocks, and implements a private API server for provers to interact with.
    - `Witness Generator`服务（`core/bin/zksync_witness_generator`）创建Prover所需块证明的输入数据，并实现用于证明生成器交互的私有API服务器。

Thus, in order to get a local zkSync setup running, the following has to be done:
因此，要启动本地zkSync设置，必须执行以下操作：

- zkSync smart contract is compiled and deployed to the Ethereum.
- 编译zkSync智能合约并将其部署到以太坊上。
- zkSync server is launched.
- 启动zkSync服务器。
- At least one prover is launched and connected to the Server application.
- 启动至少一个证明生成器并将其连接到Server应用程序。

## Low-Level Overview 细节概述

This section provides an overview on folders / sub-projects that exist in this repository.
本节提供了有关此存储库中存在的文件夹/子项目的概述。

- `/bin`: Infrastructure scripts which help to work with zkSync applications.
- `/bin`：基础架构脚本，可帮助处理zkSync应用程序。
- `/contracts`: Everything related to zkSync smart-contract.
- `/contracts`：与zkSync智能合约相关的所有内容。
  - `/contracts`: Smart contracts code
  - `/contracts`：智能合约代码
  - `/scripts` && `/src.ts`: TypeScript scripts for smart contracts management.
  - `/scripts`和`/src.ts`：智能合约管理的TypeScript脚本。
- `/core`: Code of the sub-projects that implement zkSync network.
- `/core`：实现zkSync网络的子项目的代码。
  - `/bin`: Applications mandatory for zkSync network to operate.
  - `/bin`：zkSync网络操作所必需的应用程序。
    - `/server`: zkSync server application.
    - `/server`：zkSync服务器应用程序。
    - `/prover`: zkSync prover application.
    - `/prover`：zkSync证明生成器应用程序。
    - `/data_restore`: Utility to restore a state of the zkSync network from a smart contract.
    - `/data_restore`：从智能合约恢复zkSync网络状态的实用程序。
    - `/key_generator`: Utility to generate verification keys for network.
    - `/key_generator`：生成网络验证密钥的实用程序。
    - `/parse_pub_data`: Utility to parse zkSync operation pubdata.
    - `/parse_pub_data`：解析zkSync操作pubdata的实用程序。
    - `/zksync_core`: zkSync server Core microservice.
    - `/zksync_core`：zkSync服务器核心微服务。
    - `/zksync_api`: zkSync server API microservice.
    - `/zksync_api`：zkSync服务器API微服务。
    - `/zksync_eth_sender`: zkSync server Ethereum sender microservice.
    - `/zksync_eth_sender`：zkSync服务器以太坊发送方微服务。
    - `/zksync_witness_generator`: zkSync server Witness Generator & Prover Server microservice.
    - `/zksync_witness_generator`：zkSync服务器证明生成器和证明生成器服务器的微服务。
  - `/lib`: Dependencies of the binaries above.
  - `/lib`：上述二进制文件的依赖项。
    - `/basic_types`: Crate with declaration of the essential zkSync primitives, such as `address`.
    - `/basic_types`：声明zkSync基本原语（例如`address`）的包。
    - `/circuit`: Cryptographic environment enforsing the correctness of executed transactions in the zkSync network.
    - `/circuit`：在zkSync网络中执行的交易的正确性的加密环境。
    - `/config`: Utilities to load configuration options of zkSync applications.
    - `/config`：加载zkSync应用程序配置选项的实用程序。
    - `/contracts`: Loaders for zkSync contracts interfaces and ABI.
    - `/contracts`：zkSync合约接口和ABI的加载器。
    - `/crypto`: Cryptographical primitives using among zkSync crates.
    - `/crypto`：在zkSync包之间使用的加密原语。
    - `/eth_client`: Module providing an interface to interact with an Ethereum node.
    - `/eth_client`：提供与以太坊节点交互的接口的模块。
    - `/prometheus_exporter`: Prometheus data exporter.
    - `/prometheus_exporter`：Prometheus数据导出程序。
    - `/prover_utils`: Utilities related to the proof generation.
    - `/prover_utils`：与证明生成相关的实用程序。
    - `/state`: A fast pre-circuit executor for zkSync transactions used on the Server level to generate blocks.
    - `/state`：用于在服务器级别上生成块的zkSync交易的快速预电路执行器。
    - `/storage`: An encapsulated database interface.
    - `/storage`：封装的数据库接口。
    - `/types`: zkSync network operations, transactions and common types.
    - `/types`：zkSync网络操作、交易和通用类型。
    - `/utils`: Miscellaneous helpers for zkSync crates.
    - `/utils`：zkSync包的杂项辅助程序。
    - `/vlog`: An utility library for verbose logging.
    - `/vlog`：用于详细记录的实用程序库。
  - `/tests`: Testing infrastructure for zkSync network.
  - `/tests`：zkSync网络的测试基础设施。
    - `/loadnext`: An application for highload testing of zkSync server.
    - `/loadnext`：用于zkSync服务器高负载测试的应用程序。
    - `/test_account`: A representation of zkSync account which can be used for tests.
    - `/test_account`：可用于测试的zkSync账户表示。
    - `/testkit`: A relatively low-level testing library and test suite for zkSync.
    - `/testkit`：用于zkSync的相对低级别测试库和测试套件。
    - `/ts-tests`: Integration tests set implemented in TypeScript. Requires a running Server and Prover applications to operate.
    - `/ts-tests`：在TypeScript中实现的集成测试集。需要运行的服务器和证明生成器应用程序才能操作。
- `/docker`: Dockerfiles used for development of zkSync and for packaging zkSync for a production environment.
- `/docker`：用于zkSync开发和打包zkSync以供生产环境使用的Dockerfiles
- `/etc`: Configration files.
- `/etc`: 配置文件
  - `/env`: `.env` files that contain environment variables for different configuration of zkSync Server / Prover.
  - `/env`：包含不同的zkSync服务器/证明程序配置的环境变量的`.env`文件。
  - `/js`: Configuration files for JavaScript applications (such as Explorer).
  - `/js`：JavaScript应用程序（如Explorer）的配置文件。
  - `/tokens`: Configuration of supported Ethereum ERC-20 tokens.
  - `/tokens`：支持的以太坊ERC-20代币的配置。
- `/infrastructure`: Application that aren't naturally a part of zkSync core, but are related to it.
- `/infrastructure`：与zkSync核心相关但不是自然部分的应用程序。
- `/keys`: Verification keys for `circuit` module.
- `/keys`：`circuit`模块的验证密钥。
- `/sdk`: Implementation of client libraries for zkSync network in different programming languages.
- `/sdk`：使用不同编程语言实现zkSync网络客户端库。
  - `/zksync-crypto`: zkSync network cryptographic primitives, which can be compiled to WASM.
  - `/zksync-crypto`：zkSync网络的加密原语，可编译为WASM。
  - `/zksync.js`: A JavaScript / TypeScript client library for zkSync.
  - `/zksync.js`：JavaScript/TypeScript的zkSync客户端库。
  - `/zksync-rs`: Rust client library for zkSync.
  - `/zksync-rs`：Rust的zkSync客户端库。
