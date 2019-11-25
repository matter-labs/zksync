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

## async getEthereumBalance

Method similar to `syncWallet.getBalance` but used to query balance in the Ethereum network.

### Signature
```typescript
export async function getEthereumBalance(
    ethSigner: ethers.Signer,
    token: Token
): Promise<utils.BigNumber>;
```

### Inputs and outputs

| Name | Description | 
| -- | -- |
| ethSigner | `Signer` from `ethers.js`, should be connected to ethereum node. |
| token | token of interest, "ETH" or address of the supported ERC20 token |
| returns | Balance of this token | 

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

# Types

## SyncAddress

Length of address is 20 bytes, represented as `0x`-prefixed, hex-encoded string(e.g. `0x2d5bf7a3ab29f0ff424d738a83f9b0588bc9241e`).

### Definition
```typescript
export type SyncAddress = string;
```

## Token

Token is ETH or address of corresponding ERC20 contract(e.g. `0xdAC17F958D2ee523a2206206994597C13D831ec7`).

### Definition
```typescript
export type Token = "ETH" | string;
```

## SyncAccountState

State of the Sync account. Committed state corresponds to the most resent state that is not yet verified.
Verified state is account state that was verified.

Account `id` is numerical identifier of the account is the Sync network.

### Definition
```typescript
export interface SyncAccountState {
    address: SyncAddress;
    id?: number;
    committed: {
        balances: {
            [token: string]: utils.BigNumberish;
        };
        nonce: number;
    };
    verified: {
        balances: {
            [token: string]: utils.BigNumberish;
        };
        nonce: number;
    };
}
```


## SyncCloseAccount

Signed account close transaction.

### Definition
```typescript
export interface SyncCloseAccount {
    account: SyncAddress;
    nonce: number;
    signature: Signature;
}
