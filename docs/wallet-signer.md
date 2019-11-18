# Intro 

In order to have account in the Sync network user has to have public and secret keys generated from random seed.
This keys are used to authenticate Sync transactions. Keys can be generated from random byte array. For convenience
user can derive Sync key pair from ethereum wallet(`Signer` from `ethers.js` ), this way there is one way
mapping between ethereum wallet and Sync wallet. 

`SyncSigner` is used to store keys and signing transaction. `SyncWallet` integrates `SyncSigner` and provides 
simple API for sending transaction in the Sync network. 

# class SyncWallet

## constructor

### Signature
```typescript
constructor(signer: SyncSigner, provider: SyncProvider, ethProxy: ETHProxy);
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| signer | Sync signer that will be used for transaction signing.|
| provider | Sync provider that is used for submitting transaction to the Sync network. |
| ethProxy | Ethereum proxy that is used for read-only access to the ethereum network. |

## async syncTransfer

Moves funds between accounts inside Sync network.

### Signature
```typescript
async syncTransfer(
    to: SyncAddress,
    token: Token,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: "commited" | number = "commited"
): Promise<TransactionHandle>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| to | Sync address of the recipient of funds |
| token | token to be transfered ("ETH" or address of the ERC20 token) |
| amount | amount of token to be transferred |
| fee | amount of token to be payed as a fee for this transaction |
| nonce | Nonce that is going to be used for this transaction. ("commited" is used for the last known nonce for this account) |
| returns | Handle of the submitted transaction | 


## async withdrawTo

Moves funds from the Sync account to ethereum address.

### Signature
```typescript
async withdrawTo(
    ethAddress: string,
    token: Token,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: "commited" | number = "commited"
): Promise<TransactionHandle>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| ethAddress | ethereum address of the recipient |
| token | token to be transfered ("ETH" or address of the ERC20 token) |
| amount | amount of token to be transferred |
| fee | amount of token to be payed as a fee for this transaction |
| nonce | Nonce that is going to be used for this transaction. ("commited" is used for the last known nonce for this account) |
| returns | Handle of the submitted transaction | 

# async function depositFromETH

Moves funds from ethereum account(represented as `Signer` from `ethers.js`) to the Sync account.
Fees are payed by ethereum account in ETH currency. Fee should be >=  base fee, calculated on the contract based on the 
current gas price. 

Formula for base fee calculation:
| ETH token | 2 * 179000 * GAS_PRICE |
| ERC20 token | 2 * 214000 * GAS_PRICE |

### Signature

```typescript
async function depositFromETH(
    depositFrom: ethers.Signer,
    depositTo: SyncWallet,
    token: Token,
    amount: utils.BigNumberish,
    maxFeeInETHCurrenty: utils.BigNumberish
);
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| depositFrom | ethereum account of the sender |
| depositTo | Sync account of the receiver |
| token | token to be transfered ("ETH" or address of the ERC20 token) |
| amount | amount of token to be transferred |
| fee | amount of `ETH` to be payed by `depositFrom` wallet as a fee for this transaction |
