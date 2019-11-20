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

## SyncTransfer

Signed transfer transaction.

### Definition
```typescript
export interface SyncTransfer {
    from: SyncAddress;
    to: SyncAddress;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
}
```

## SyncWithdraw

Signed withdraw transaction.

### Definition
```typescript
export interface SyncWithdraw {
    account: SyncAddress;
    ethAddress: string;
    token: number;
    amount: utils.BigNumberish;
    fee: utils.BigNumberish;
    nonce: number;
    signature: Signature;
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
```

## BlockInfo

### Definition
```typescript
export interface BlockInfo {
    blockNumber: number;
    committed: boolean;
    verified: boolean;
}
```

## SyncTxReceipt

### Definition
```typescript
export interface SyncTxReceipt {
    executed: boolean;
    success?: boolean;
    failReason?: string;
    block?: BlockInfo;
}
```

## SyncPriorityOperationReceipt

### Definition
```typescript
export interface SyncPriorityOperationReceipt {
    executed: boolean;
    block?: BlockInfo;
}
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
