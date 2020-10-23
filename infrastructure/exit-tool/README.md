# zkSync Exit Tool

This tool is capable of generating input data for exit transaction for zkSync exodus mode.

## Prerequisites

- `Docker` and `docker-compose`.
- 20+ GB of free space. In order to create an exit proof, the universal cryptographical setup must be downloaded (~8GB), and besides that there should be enough space to fit the whole zkSync chain.
- Access to the Web3 API (e.g. provided by Ethereum node or Infura) in order to gather data from Ethereum blockchain.

## Mechanics

In order to create exit proof, the following steps must be done:

- Download the universal cryptographical setup.
- Initialize PostgreSQL database to store zkSync network data (blocks, transactions, etc).
- Restore the network state from the smart contract on Ethereum.
- Generate proof for user's exit balance.

This tool handles these steps as follows:

- PostgreSQl database is initialized via docker-compose with data folder mounted to the host system folder `./volumes`. Mounting to the host system is required to not lose the partially synchronized state between launches.
- Cryptographical setup is downloaded upon first launch into the local folder `./setup`. Its size is roughly 8GB and this operation will only be done once.
- Restoring the network state and the exit proof generating are encapsulated into a docker container.

## Usage

Prior to the state restoring, application must be initialized

```sh
./exit-tool.sh init
```

At this step, database will be created and initialized.

After that, you can launch the utility:

```sh
./exit-tool.sh run NETWORK ACCOUNT_ID TOKEN WEB3_ADDR
```

where:

- NETWORK: Ethereum network to use. Must be one of `rinkeby`, `ropsten` or `mainnet`.
- ACCOUNT_ID: A numerical identifier of account in the zkSync network.
- TOKEN: Token to be withdrawn. Must be either numerical token ID or ERC-20 token address.
- WEB3_ADDR: Address of the Web3 HTTP API.

Example:

```sh
./exit-tool.sh run rinkeby 1 0 http://127.0.0.1:8545
```

In this example, we use Rinkeby Ethereum testnet, generate a proof for account with ID 1 and token with ID 0 (Ether), and use the API located at http://127.0.0.1:8545

**Note:** Synchronizing the state will scan a big part of Ethereum blockchain, and that's a lot of work to do. It may take hours or even days to complete, depending on the size of zkSync blockchain.

However, if the synchronization process was interrupted, it is possible to resume a previously started data restore:

```sh
./exit-tool.sh continue rinkeby 1 0 http://127.0.0.1:8545
```

In that case, a partially restored state will be loaded from the database, and restoring will continue from this point.

Once the state is restored, tool will generate an exit proof and will print it to the console. Output may look roughly as follows:

```json
{
  "token_id": 0,
  "account_id": 1,
  "account_address": "0x3b48b21a2f4910c04c04de00a23f7c07bf3cb04f",
  "amount": "3939999843080000000000",
  "proof": {
    "inputs": [
      "0x11e55c73db5f552b9d95b3351a90165676da2af365be22721e874448bb47c6ca"
    ],
    "proof": [
      "0x314676cac431331aacfab085471f78e5dd4151c886f83a342a9e8aad7064eb2",
      "0x1a6147ba1176be942b8b1abcc347f91de54955a3cf87726bdd99050edba2d01",
      "0xaf47b6b53b978235a0ef6b272c7c14bda8d1026fa62ccbff30e3d6c3dc0c04",
      "0x2d5ce349718a2a0659fc67a9bcbbdbbad99401ec4481f4eb0ed2c4326f381f15",
      "0xbb038dfae788d2e2b560a70ec523f5028e9bbef9149652827ea3a8289e64c03",
      "0x1c3e60e60a8f1c570934bdd7534efb6bceab33d0e9ab69b809139e314573ed41",
      "0x2cee6e92d4f6161582392dbcf4ba48290f9d1dd2d4aed8ffafe4cce3e4839038",
      "0xcfe36c8728e769d78890a19f8a387bc6a2c8d6d76d74f1448f50ea70fea0ab3",
      "0x1f7bbaf4d632ab00fc88cdc0261b34537f7dd6019e2e9b0a445b189a26d5d46a",
      "0x51defabf221112b25b2b99e6be0caf189becd4ed85adb2a128143880b2250f0",
      "0x21c098c20f968d5dfb6d81c45b8412fc66c5323e51a73a76231472dd9ca1ae64",
      "0xd1a678899eac7b6c6f1e0e363a833e4a7861f35c326b7d5355e2682cd088775",
      "0x2058b084ea1e288c81ba8ef4df434d9a4e03e32ec821a1be468af68beaa36c83",
      "0x1765d6a52744edb661c8c8e7fe80b781c0690b3d21b98bdb0c0d0fb723222cbc",
      "0x1560b3bb23b8b25f4f58acf6d457f7d53f87e0aef30b3ac642bbf3cfef88f794",
      "0x1fc0fd25de54736625bbbaed6a336ea24c132c7b771d712c42bc7f257fc31f9b",
      "0x2b839102ae0a6679ad6ea45c818022155a5e0392f582cd4b68d4a58d1465a1a1",
      "0x6fcb21b6a5583e7b60d5404bae67799a28b51bbacb586d5ce7a218133390ccd",
      "0x1ce7f02c0ff33b532ed26850162e6f1289b6884bd920471b3bb283c044531585",
      "0x1ce956bdc735bd5b5d947c5ea63e94923aea7fcf56042dd90d1c5d777c497c81",
      "0x6b927b847965eac8bbe7603c7729c61c5ca41ce2b6115d1dd9b05f6d9f5c134",
      "0x25d819ef472dfcf62a469acaf28ebf85ffd681c6b7d34e0c0466d9ab1b372d86",
      "0x14fa12dc426ce6038387e068ce63a512e0e08a84e69b9cbaa17f1a558a93b0f",
      "0x304753ce7297dbbe83013b18cac18e962f2601eaf6c898ada51c79c551cd572f",
      "0x24a7674aa7d5f5cc02b0858481876cfbb026002bab5a98bf589479fc9304a2ee",
      "0x12a64be6750dd39bf5fabb77aa24018144f6d850384340d860c59514c2aa440f",
      "0x4d70b5a1dde6e2f397dd780ed6e7144f00807058f58775cd6f18b59a47dbec3",
      "0xf41386d588d768d8d74ce6394eb1ef064cebf3dfd315bc2fbd0a16f0c99df03",
      "0x11f3cd395d2695f2b1dd3a4e3702ccaeedc62c6e551fe9f62594aa05439dc7",
      "0x2f493446b767bf41c3ea5360d0cf5262357040e8f585daedae8dff5bdf2a8858",
      "0x2c4a32600012042c46c64ad6ecc56abf5dc41e120de5ddc04dbf62fe4128da0e",
      "0x26c07e32e6d267bf323ca6b4912a24bc667a0f5fabd9e20e1eacc84ce94af511",
      "0x39905b1b49147ee1237722580e881d9b0956d069266e932b5341c2c76b3046e"
    ]
  }
}
```

This data represents inputs for an [`exit` method on the smart contract](https://github.com/matter-labs/zksync/blob/e3ee657e5f02601e0aed523f4237cc9708d6daf9/contracts/contracts/ZkSync.sol#L351).

What user has to do after that:

1. Create transaction for an `exit` method call, sign it and broadcast to register balance to withdraw.
2. Send a transaction which invokes either [`withdrawETH`](https://github.com/matter-labs/zksync/blob/e3ee657e5f02601e0aed523f4237cc9708d6daf9/contracts/contracts/ZkSync.sol#L175) or [`withdrawERC20`](https://github.com/matter-labs/zksync/blob/e3ee657e5f02601e0aed523f4237cc9708d6daf9/contracts/contracts/ZkSync.sol#L202) method of contract to obtain their funds.
