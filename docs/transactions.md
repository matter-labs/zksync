# Intro 

There are two types of transactions:
1) *Transactions* They are submitted directly to the Sync network.
Transfer, withdraw and account close are transactions.
2) *Priority operations* They are initiated with ethereum transaction.
Deposits and emergency withdraws are priority operations.

# Types

# class Wallet

## async syncTransfer

Moves funds between accounts inside Sync network.

{% hint style="alert" %}
Transfer amount and fee should have limited number of significant digits according to spec.
{% endhint %}


### Signature
```typescript
async syncTransfer(
    to: Address,
    token: Token,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: "committed" | number = "committed"
): Promise<Transaction>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| to | Sync address of the recipient of funds |
| token | token to be transfered ("ETH" or address of the ERC20 token) |
| amount | amount of token to be transferred |
| fee | amount of token to be payed as a fee for this transaction |
| nonce | Nonce that is going to be used for this transaction. ("committed" is used for the last known nonce for this account) |
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
    nonce: "committed" | number = "committed"
): Promise<TransactionHandle>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| ethAddress | ethereum address of the recipient |
| token | token to be transfered ("ETH" or address of the ERC20 token) |
| amount | amount of token to be transferred |
| fee | amount of token to be payed as a fee for this transaction |
| nonce | Nonce that is going to be used for this transaction. ("committed" is used for the last known nonce for this account) |
| returns | Handle of the submitted transaction | 


# async function depositFromETH

Moves funds from ethereum account(represented as `Signer` from `ethers.js`) to the Sync account.
Fees are payed by ethereum account in ETH currency. Fee should be >=  base fee, calculated on the contract based on the 
current gas price. 

Formula for base fee calculation:
| Token | Formula |
| -- | -- |
| ETH token | `2 * 179000 * GAS_PRICE` |
| ERC20 token | `2 * 214000 * GAS_PRICE` |

### Signature

```typescript
async function depositFromETH(
    depositFrom: ethers.Signer,
    depositTo: Wallet,
    token: Token,
    amount: utils.BigNumberish,
    maxFeeInETHCurrenty: utils.BigNumberish
): Promise<ETHOperation>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| depositFrom | ethereum account of the sender |
| depositTo | Sync account of the receiver |
| token | token to be transferred ("ETH" or address of the ERC20 token) |
| amount | amount of token to be transferred |
| maxFeeInETHCurrenty | amount of `ETH` to be payed by `depositFrom` wallet as a fee for this transaction |
| returns | Handle for this transaction. | 

# async function emergencyWithdraw

If ordinary withdraw from Sync account is ignored by network operators user could create emergency 
withdraw request using special ethereum transaction, this withdraw request can't be ignored.

Moves full amount of the given token from the Sync account to ethereum account(represented as `Signer` from `ethers.js`).

Fees are payed by ethereum account in ETH currency. Fee should be >=  base fee, calculated on the contract based on the 
current gas price. 
Formula for base fee calculation: ```2 * 170000 * GAS_PRICE```

### Signature

```typescript
export async function emergencyWithdraw(
    withdrawTo: ethers.Signer,
    withdrawFrom: Wallet,
    token: Token,
    maxFeeInETHCurrenty: utils.BigNumberish,
    nonce: "committed" | number = "committed"
): Promise<ETHOperation>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| withdrawTo | ethereum account of the receiver, also this account posts withdraw request to ethereum |
| withdrawFrom | Sync account of the sender |
| token | token to be transferred ("ETH" or address of the ERC20 token) |
| amount | amount of token to be transferred |
| maxFeeInETHCurrenty | amount of `ETH` to be payed by `withdrawTo` wallet as a fee for this transaction |
| returns | Handle for this transaction. | 

# class Transaction

Sync transaction object that is used for tracking progress of the recently created sync transactions.

It can be in the following states 

States:
| Name | Description |
| -- | -- |
| Sent | Default state after transaction is submitted to the Sync network. |
| Commited | Transaction was included to the Sync network block |
| Verified | Corresponding Sync network block was verified |

## async awaitReceipt

Returns when transaction was included to the Sync network block.

### Signature 

```typescript
async awaitReceipt();
```

## async awaitVerifyReceipt

Returns when transaction block was verified in the Sync network.

### Signature 

```typescript
async awaitVerifyReceipt();
```

# class ETHOperation

Sync priority operation object, used for tracking progress of the recently created priority operations.
Priority operation is initiated by ethereum transaction.

Most of the time user in interested in the `waitCommit` or `waitVerify` methods.

It can be in the following states 

States:
| Name | Description |
| -- | -- |
| Sent | Default state after transaction is submitted to ethereum. |
| Mined | After ethereum transaction was mined |
| Commited | Priority operation was included to the Sync network block |
| Verified | Corresponding Sync network block was verified |

## async awaitEthereumTxCommit

Returns after etherum transaction was mined.

### Signature 

```typescript
async awaitEthereumTxCommit();
```

## async awaitReceipt

Returns when priority operation was included to the Sync network block.

### Signature 

```typescript
async awaitReceipt();
```

## async awaitVerifyReceipt

Returns when block with this operation was verified in the Sync network.

### Signature 

```typescript
async awaitVerifyReceipt();
```

# Utils

## closestPackableTransactionAmount

All transfers amounts should be packable to 5-byte long floating point representation.
This functions is used to check if this amount can be used as a transfer amount.

### Signature
```typescript
export function closestPackableTransactionAmount(
    amount: utils.BigNumberish
): utils.BigNumber;
```

## closestPackableTransactionFee

All fees payed in transfers and withdraws should be packable to 2-byte long floating point representation.
This functions is used to check if this amount can be used as a fee.

### Signature
```typescript
export function closestPackableTransactionFee(
    fee: utils.BigNumberish
): utils.BigNumber;
```

# Types

## BlockInfo

### Definition
```typescript
export interface BlockInfo {
    blockNumber: number;
    committed: boolean;
    verified: boolean;
}
```

## TxReceipt

### Definition
```typescript
export interface TxReceipt {
    executed: boolean;
    success?: boolean;
    failReason?: string;
    block?: BlockInfo;
}
```

## PriorityOperationReceipt

### Definition
```typescript
export interface PriorityOperationReceipt {
    executed: boolean;
    block?: BlockInfo;
}
```

## Signature

Signature contains public key for which this signature should be valid.

`pubKey` and `signature` fields are represented as hex-enoded byte arrays.

`pubKey` is 32-byte long compressed elliptic curve point.

`signature` is 64-byte long array. First 32 bytes is compressed `r` point and other 32 bytes is `s`
scalar in little-endian representation.

### Definition
```typescript
export interface Signature {
    pubKey: string;
    signature: string;
}
```

## Transfer

Signed transfer transaction.

### Definition
```typescript
export interface Transfer {
    from: Address;
    to: Address;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
}
```

## Withdraw

Signed withdraw transaction.

### Definition
```typescript
export interface Withdraw {
    account: Address;
    ethAddress: string;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
}
```
