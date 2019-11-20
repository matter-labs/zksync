# Intro 

In order to have account in the Sync network user has to have public and secret keys generated from random seed.
This keys are used to authenticate Sync transactions. Keys can be generated from random byte array. For convenience
user can derive Sync key pair from ethereum wallet(`Signer` from `ethers.js` ), this way there is one way
mapping between ethereum wallet and Sync wallet. 

`SyncSigner` is used to store keys and signing transaction. `SyncWallet` integrates `SyncSigner` and provides 
simple API for sending transaction in the Sync network. 

Transaction handles (`ETHOperationHandle` and `TransactionHandle`) are used to provide simple API for tracking 
progress of recently submitted transactions.

There are two types of transactions:
1) *Transactions* They are submitted directly to the Sync network.
Transfer, withdraw and account close are transactions.
2) *Priority operations* They are initiated with ethereum transaction.
Deposits and emergency withdraws are priority operations.

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

## static async fromEthWallet

### Signature
```typescript
static async fromEthWallet(
    ethWallet: ethers.Signer,
    provider: SyncProvider,
    ethProxy: ETHProxy
): Promise<SyncWallet>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| ethWallet | `Signer` from `ethers.js` that is used to created random seed for `SyncSigner`|
| provider | Sync provider that is used for submitting transaction to the Sync network. |
| ethProxy | Ethereum proxy that is used for read-only access to the ethereum network. |
| returns | `SyncWallet` derived from ethereum wallet |

## async syncTransfer

Moves funds between accounts inside Sync network.

{% hint style="alert" %}
Transfer amount and fee should have limited number of significant digits according to spec.
{% endhint %}


### Signature
```typescript
async syncTransfer(
    to: SyncAddress,
    token: Token,
    amount: utils.BigNumberish,
    fee: utils.BigNumberish,
    nonce: "committed" | number = "committed"
): Promise<TransactionHandle>;
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

## async close

Removes account from the Sync network.

### Signature
```typescript
async close(
    nonce: "committed" | number = "committed"
): Promise<TransactionHandle>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| nonce | Nonce that is going to be used for this transaction. ("committed" is used for the last known nonce for this account) |
| returns | Handle of the submitted transaction | 

## async getAccountState

### Signature
```typescript
async getAccountState(): Promise<SyncAccountState>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| returns | State of the given account, see [types](types-utils.md) for detailed description.  | 

## async getBalance

### Signature
```typescript
async getBalance(
    token: Token,
    type: "committed" | "verified" = "committed"
): Promise<utils.BigNumber>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| token | token of interest, "ETH" or address of the supported ERC20 token |
| type | "committed" or "verified" |
| returns | Balance of this token | 

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
    depositTo: SyncWallet,
    token: Token,
    amount: utils.BigNumberish,
    maxFeeInETHCurrenty: utils.BigNumberish
): Promise<ETHOperationHandle>;
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
    withdrawFrom: SyncWallet,
    token: Token,
    maxFeeInETHCurrenty: utils.BigNumberish,
    nonce: "committed" | number = "committed"
): Promise<ETHOperationHandle>;
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

# class TransactionHandle

Sync transaction handle, used for tracking progress of the recently created sync transactions.

It can be in the following states 

States:
| Name | Description |
| -- | -- |
| Sent | Default state after transaction is submitted to the Sync network. |
| Commited | Transaction was included to the Sync network block |
| Verified | Corresponding Sync network block was verified |

## async waitCommit

Returns when transaction was included to the Sync network block.

### Signature 

```typescript
async waitCommit();
```

## async waitVerify

Returns when transaction block was verified in the Sync network.

### Signature 

```typescript
async waitVerify();
```

# class ETHOperationHandle

Sync priority operation handle, used for tracking progress of the recently created priority operations.
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

## async waitTxMine

Returns after etherum transaction was mined.

### Signature 

```typescript
async waitTxMine();
```

## async waitCommit

Returns when priority operation was included to the Sync network block.

### Signature 

```typescript
async waitCommit();
```

## async waitVerify

Returns when block with this operation was verified in the Sync network.

### Signature 

```typescript
async waitVerify();
```
# class SyncSigner

## static fromPrivateKey

### Signature

```typescript
static fromPrivateKey(pk: BN): SyncSigner;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| pk | private key |
| returns | `SyncSigner` derived from private key | 

## static fromSeed

### Signature

```typescript
static fromSeed(seed: Buffer): SyncSigner;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| seed | Random bytes array (should be >= 32 bytes long) |
| returns | `SyncSigner` derived from this seed | 

## address

### Signature

```typescript
address(): SyncAddress;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| returns | Address of the Sync account derrived from corresponding public key | 

## signSyncTransfer

Signs transfer transaction, result can be submitted to the Sync network.
Sender for this transaction is assumed to be this `SyncSigner` address.

### Signature

```typescript
signSyncTransfer(transfer: {
    to: SyncAddress;
    tokenId: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
}): SyncTransfer;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| transfer.to | Address of the recipient | 
| transfer.tokenId | Numerical token id  | 
| transfer.amount | Amount to transfer, payed in token  | 
| transfer.fee | Fee to pay for transfer, payed in token  | 
| transfer.nonce | Transaction nonce   | 
| returns | Signed Sync transfer transaction | 

## signSyncWithdraw

Signs withdraw transaction, result can be submitted to the Sync network.
Sender for this transaction is assumed to be this `SyncSigner` address.

### Signature

```typescript
signSyncWithdraw(withdraw: {
    ethAddress: string;
    tokenId: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
}): SyncWithdraw {
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| withdraw.ethAddress | Ethereum address of the recipient | 
| withdraw.tokenId | Numerical token id  | 
| withdraw.amount | Amount to withdraw, payed in token  | 
| withdraw.fee | Fee to pay for withdraw, payed in token  | 
| withdraw.nonce | Transaction nonce   | 
| returns | Signed Sync withdraw transaction | 

## signSyncCloseAccount

Signs account close transaction, result can be submitted to the Sync network.
Account to be closed is assumed to be this `SyncSigner` address.

### Signature

```typescript
signSyncCloseAccount(close: { nonce: number }): SyncCloseAccount;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| close.nonce | Transaction nonce   | 
| returns | Signed Sync account close transaction | 

## syncEmergencyWithdrawSignature

Signs emergency withdraw transaction, returned signature can be used to submit withdraw request to ethereum network.
Account for withdraw is assumed to be this `SyncSigner` address.

### Signature

```typescript
syncEmergencyWithdrawSignature(emergencyWithdraw: {
    ethAddress: string;
    tokenId: number;
    nonce: number;
}): Buffer;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| emergencyWithdraw.ethAddress | Ethereum address of the recipient | 
| emergencyWithdraw.tokenId | Numerical token id in the SyncNetwork | 
| emergencyWithdraw.nonce | Transaction nonce   | 
| returns | Signature for emergency withdraw transaction (64 byte, packed) | 
