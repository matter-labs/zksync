# zkSync Exit Tool

This tool is capable of generating input data for exit transaction for zkSync exodus mode.

## Prerequisites

- `Docker` and `docker-compose`.
- 20+ GB of free space. In order to create an exit proof, the universal cryptographical setup must be downloaded (~8GB),
  and besides that there should be enough space to fit the whole zkSync chain.
- Access to the Web3 API (e.g. provided by Ethereum node or Infura) in order to gather data from Ethereum blockchain.

## Mechanics

In order to create exit proof, the following steps must be done:

- Download the universal cryptographical setup.
- Initialize PostgreSQL database to store zkSync network data (blocks, transactions, etc).
- Restore the network state from the smart contract on Ethereum.
- Generate proof for user's exit balance.

This tool handles these steps as follows:

- PostgreSQl database is initialized via docker-compose with data folder mounted to the host system folder `./volumes`.
  Mounting to the host system is required to not lose the partially synchronized state between launches.
- Cryptographical setup is downloaded upon first launch into the local folder `./setup`. Its size is roughly 8GB and
  this operation will only be done once.
- Restoring the network state and the exit proof generating are encapsulated into a docker container.

## Usage

Prior to the state restoring, application must be initialized.

Firstly, you need to create the `volumes/postgres` folder. Then run the initialization script with the following command

```sh
./exit-tool.sh init
```

At this step, database will be created and initialized.

After that, you can launch the utility:

```sh
./exit-tool.sh run NETWORK ACCOUNT_ADDRESS TOKEN WEB3_ADDR
```

where:

- NETWORK: Ethereum network to use. Must be one of `rinkeby`, `ropsten` or `mainnet`.
- ACCOUNT_ADDRESS: Address of the target account. **Note:** address **should not** start with `0x` prefix.
- TOKEN: Token to be withdrawn. Must be either numerical token ID or ERC-20 token address.
- WEB3_ADDR: Address of the Web3 HTTP API.

Example:

```sh
./exit-tool.sh run rinkeby 3b48b21a2f4910c04c04de00a23f7c07bf3cb04f 0 http://127.0.0.1:8545
```

In this example, we use Rinkeby Ethereum testnet, generate a proof for account with address
0x3b48b21a2f4910c04c04de00a23f7c07bf3cb04f and token with ID 0 (Ether), and use the API located at
`http://127.0.0.1:8545`

If during the process you encounter any error with the database, you should reset the database and run the
`./exit-tool init` again.

**Note:** Synchronizing the state will scan a big part of Ethereum blockchain, and that's a lot of work to do. It may
take hours or even days to complete, depending on the size of zkSync blockchain.

However, if the synchronization process was interrupted, it is possible to resume a previously started data restore:

```sh
./exit-tool.sh continue rinkeby 3b48b21a2f4910c04c04de00a23f7c07bf3cb04f 0 http://127.0.0.1:8545
```

In that case, a partially restored state will be loaded from the database, and restoring will continue from this point.

Once the state is restored, tool will generate an exit proof and will print it to the console. Output may look roughly
as follows:

```json
{
  "storedBlockInfo": {
    "blockNumber": 2235,
    "priorityOperations": 0,
    "pendingOnchainOperationsHash": "0xc5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470",
    "timestamp": 1618987864,
    "stateHash": "0x0c44d4ed6eecba6565ab63bf713a8d7be538f98d4950f5ade4ac84a3638e8f35",
    "commitment": "0xfbdcaadca3c6cb4f0bb2eccd46218ef73cdff58c19549737d266529cad0f67e5"
  },
  "owner": "0x3b48b21a2f4910c04c04de00a23f7c07bf3cb04f",
  "accountId": 131,
  "tokenId": 0,
  "amount": "1354669000000000",
  "proof": {
    "inputs": ["0x11dc24e0520936783ef6cf2ead4a2f40057d0cee7e0c205da4e4c1f102360e1c"],
    "proof": [
      "0x1e4f4360fef1dbc0823472e8aca5d29caee1b667e0354e6747d33020fccef74b",
      "0x54a4bad356b44f710be10c07f98ac3006534fdf8147be7031071e9da1bc9781",
      "0xecb0793b066a84e07e10e82ce9153562771f15f4d2ea156f50ef23387eb3bff",
      "0x1c59efed4501a0d5d1d229dd2045e32944aadfee47a38de6b6b69b8bc5eb0189",
      "0x300f189fdd7e14addabbf29115ed23427f5e011aec358c18892452d5f93ef9b6",
      "0x1270402c986c9514a171c86888171e1396cdf2174ce5740b9b5866fb3d356496",
      "0xedaf01436e147f2fdc64de63f484ca62aee41f096dee05006274bed594a4f4",
      "0x16811befa267b39189dbcfb29b6eb9011f704bfeaa0291281ca592fa66d0b23e",
      "0x21cc1afd708038f92e05fd73944970111b77b247f65a8e15833e2e504d9387a",
      "0x24fc84db9774ee7ae556cb0f43f2ebae5a50fbe4b15b7dbd1318734af8961a42",
      "0x22a366fe8910b3892919e5382d724c89146db9adea7b18cbfbe97a65be9f02ea",
      "0x1e4a9e14d79fff26f358e752db6ca2fab34e2d4c501ec2704263bfd7a8f31677",
      "0xc92e385021ec21f399af41a7712f34fd63bbefa0f436ac626baa9f9853d1a42",
      "0x21d1bbc2fd3694e54963986c20c93b1e48fcfb9684e42ba82172107366235334",
      "0x1aaa6c36e8270b09803f852054ae6f36f61ba7f6b2c93746099ae73b3eeaac29",
      "0x9c0331f4c5bd6a5b5c553751d615a2d546deed267de537e7173808a7d707c5d",
      "0x2db349b9f5b1ff0e3be0c64689a197b223219def5671132d2371ed1aef23c8f0",
      "0x16b18f25825b2384e0fdff124ee8905b54cc0bac426ecdbc1809b0b80c378700",
      "0x2d1d952c9b4583c1fb2447ebe058123db8b96f8eb830b063ba4d54722a7b18c7",
      "0x1a6ea76c5f44d13e6f03a3bcc0282c2f8f7a84e3481ee56b599eaa415845cd3a",
      "0x2e285b6e0facccce57fdae9f52d52fa221563f02cdc1667474a655c5a05fdd94",
      "0xcd06119e20786a41ff3d435e21972631c4e3a506c67231e54c7ed225dfe49e1",
      "0x1833f5b3945b223c2d58f311dc9fd33f4df20b063280071242a8021cec8c7092",
      "0x204af9a284dc8eca3223b72a93a5e05e05170aab99ecaf5c8d068e9d5f4510c3",
      "0x2799be4725b38772e0d1d843bdf60cf167a31cbd764ff2119417cc674e2fd5e3",
      "0x1aacc735b90ecd492569eca265c42de81120dc8dfb9352fe3f9270f312dd9454",
      "0xbd93eb76f83f9c02f1437a85bd21f792eaec9014a82ffbb9ab6e9e937ef4c94",
      "0x1224ac8ef5f98d19eca09475d75b2819302cc0b903ade1eb897ff0c346450fb9",
      "0x2845b0204d84a81de3321111680197521fec98c857cd2f7042d96c530721ca16",
      "0x1783b195d08ac2b597901dd71a67e579a3965dc201c4acbfd8ee8f25e0f24aab",
      "0x9fbdff9a803cf76b8031e06ee015ea52a0a019eac47e4b7a0c91c8da2f6262f",
      "0x46e53f6724dcdb913612c66b8b3cfa95cdf37d47fcc4bb6dc53734ca76c37eb",
      "0x20690079f9a7cbbf1dc1dcffe6395ea4c88e99e2bd9564c495ab72556fee2897"
    ]
  },
  "tokenAddress": "0x0000000000000000000000000000000000000000"
}
```

Data until the last entry represents inputs for an
[`performExodus` method on the smart contract](https://github.com/matter-labs/zksync/blob/master/contracts/contracts/ZkSync.sol#L574).

The last entry (`token_address`) is needed for the [`withdrawPendingBalance`][wd] method invocation;

What user has to do after that:

1. Create transaction for an [`performExodus`][pe] method call, sign it and broadcast to register balance to withdraw.
2. Send a transaction which invokes [`withdrawPendingBalance`][wd] method of contract to obtain their funds.

[pe]:
  https://github.com/matter-labs/zksync/blob/5f47fe9990ec87e3087d32d083d13e6cab331ff1/contracts/contracts/ZkSync.sol#L574
[wd]: https://github.com/matter-labs/zksync/blob/master/contracts/contracts/ZkSync.sol#L262

## Sending exodus L1 transactions

In case you have the private key for the wallet you want to perform exodus for, you may use
[`perform_exodus`](perform_exodus) script for that matter.

Otherwise, please check your wallet documentation in order to know how to execute arbitrary transactions from it.

## What if I need proofs for multiple tokens

After generating proof, run `./exit-tool.sh continue` with the new token ID. The state is already synchronized at this
moment, so it won't take as long.
