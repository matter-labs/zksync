# Getting started

## Add dependency

```bash
yarn add zksync
yarn add ethers # For interactions with ETH network
```

## Connecting to the Sync network

In order to interact with Sync network user have to know endpoint of the operator node.

```typescript
const syncProvider = await SyncProvider.newWebsocketProvider("ws://127.0.0.1:3031");

// When using WebSocket provider connection should be closed manually when needed using.
await syncProvider.disconnect();
```

Most operations require some read-only access to the Ethereum network.
We use `ethers` library to interact with Ethereum. 

Addresses of the Sync network contracts should be known in advance, for convenience now we can get this addresses 
from Sync network operator using `syncProvider`. 

```typescript
const ethersProvider = new ethers.providers.JsonRpcProvider("WEB3_URL");
const ethProxy = new ETHProxy(ethersProvider, syncProvider.contractAddress);
```

## Creating wallet

In order to use Sync network we provide `SyncWallet` object. It can be used to sign transactions 
with keys stored in `SyncSigner` and send transaction to Sync network using connection provided by `SyncProvider`.

### Creating wallet from Ethereum wallet.

For convenience user can derive Sync network account from Ethereum account. Wallet secret key will be derived from
signature of specific message.

```typescript
// Create ethereum wallet using ethers.js
const ethWallet = ethers.Wallet.fromMnemonic( MNEMONIC ).connect(ethersProvider);
```

```typescript
// Derive wallet from ethereum wallet.
const syncWallet = await SyncWallet.fromEthWallet(ethWallet, syncProvider, ethProxy);
```

## Moving funds from ethereum to the Sync network

We are going do deposit some funds from our ethereum wallet into sync account.
For that we should create specific ethereum transaction. We can create this transaction using `depositFromETH` function. 

Here we are moving "ETH" token. In order to transfer supported ERC20 token we should use ERC20 address instead of "ETH".

```typescript
const depositHandle = await depositFromETH(
    ethWallet,
    syncWallet,
    "ETH",
    utils.parseEther("1.0"),
    utils.parseEther("0.1")
);
```

After transaction is submitted to ethereum we can track its progress using returned handle.

If we want to wait until deposit is processed by the SyncNetwork.
```typescript
const depositReceipt = await depositHandle.waitCommit();
```

If we want to wait until deposit is processed and finalized using ZKP by the SyncNetwork.
```typescript
const depositReceipt = await depositHandle.waitVerify();
```

## Get balance in the Sync network

To get balance of the Sync account you can use `getBalance` method.
Committed state is last state of the account that may or may not be finalized by ZK proof.
Verified is referred to finalized by ZK proof state of the account. 

```typescript
const commitedETHBalance = await syncWallet.getBalance("ETH");
const verifiedETHBalance = await syncWallet.getBalance("ETH", "verified");
```

To get all tokens of this account you can use `getAccountState`.

```typescript
const state = await syncWallet.getAccountState("ETH");

const commitedBalances = state.committed.balances;
const commitedETHBalance = commitedBalances["ETH"];

const verifiedBalances = state.verified.balances;
const commitedETHBalance = verifiedBalances["ETH"];
```


## Moving funds inside Sync network

Let create second wallet and transfer funds to it.

```typescript
const ethWallet2 = ethers.Wallet.fromMnemonic( MNEMONIC2 ).connect(ethersProvider);
const syncWallet2 = await SyncWallet.fromEthWallet(ethWallet2, syncProvider, ethProxy);
```

To transfer funds from one Sync account to another we can use `syncTransfer` method.
We are going to transfer `0.999 ETH` to another account and pay `0.001 ETH` as a fee to operator.

```typescript
const transferHandle = await syncWallet.syncTransfer(
    syncWallet2.address(),
    "ETH",
    utils.parseEther("0.999"),
    utils.parseEther("0.001")
);
```

In order to track progress of this transaction we can use returned transaction handle.

```typescript
const transferReceipt = await depositHandle.waitCommit();
```

## Moving funds out of the Sync network

To withdraw funds from Sync account to ethereum account we can use `withdrawTo` method.

We are going to withdraw `0.998 ETH` from second sync account to the second ethereum wallet and pay `0.001 ETH` as a fee.

```typescript
const withdrawHandle = await syncWallet2.withdrawTo(
    ethWallet2.address,
    "ETH",
    ethers.utils.parseEther("0.998"),
    ethers.utils.parseEther("0.001"),
);
```

Funds will be withdrawn to the target wallet after ZKP for sync block with this operation is produced and verified.
We can wait until ZKP verification is completed using returned transaction handle. 
```typescript
await withdrawHandle.waitVerify();
```
