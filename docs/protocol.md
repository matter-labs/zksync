# zkSync Rollup Protocol

## Table of contents

- [Table of contents](#table-of-contents)
- [Glossary](#glossary)
- [Design](#design)
  - [Overview](#overview)
  - [Assumptions](#assumptions)
  - [Protocol invariant claims](#protocol-invariant-claims)
- [Data format](#data-format)
  - [Data types](#data-types)
  - [Amount packing](#amount-packing)
  - [State Merkle Tree](#state-merkle-tree)
  - [zkSync block pub data format](#zksync-block-pub-data-format)
- [zkSync operations](#zksync-operations)
  - [0. Rollup operation lifecycle](#0-rollup-operation-lifecycle)
  - [1. Noop operation](#1-noop-operation)
  - [2. Transfer](#2-transfer)
  - [3. Transfer to new](#3-transfer-to-new)
  - [4. Withdraw (Partial Exit)](#4-withdraw-partial-exit)
  - [5. Withdraw NFT](#5-withdraw-nft)
  - [6. Mint NFT](#6-mint-nft)
  - [7. Deposit](#7-deposit)
  - [8. Full exit](#8-full-exit)
  - [9. Change pubkey](#9-change-pubkey)
  - [10. Forced exit](#10-forced-exit)
  - [11. Swap](#11-swap)
- [Smart contracts API](#smart-contracts-api)
  - [Rollup contract](#rollup-contract)
    - [Deposit Ether](#deposit-ether)
    - [Deposit ERC-20 token](#deposit-erc-20-token)
    - [Withdraw Ether](#withdraw-ether)
    - [Withdraw ERC-20 token](#withdraw-erc-20-token)
    - [Censorship resistance](#censorship-resistance)
    - [Exodus mode](#exodus-mode)
    - [Rollup Operations](#rollup-operations)
  - [Governance contract](#governance-contract)
    - [Change governor](#change-governor)
    - [Add token](#add-token)
    - [Set validator](#set-validator)
    - [Check for governor](#check-for-governor)
    - [Check for active validator](#check-for-active-validator)
    - [Check that token id is valid](#check-that-token-id-is-valid)
    - [Check that token address is valid](#check-that-token-address-is-valid)
- [Block state transition circuit](#block-state-transition-circuit)
- [Appendix I: Cryptographic primitives](#appendix-i-cryptographic-primitives)
  - [Rescue hash](#rescue-hash)
  - [Bitpacking](#bitpacking)
  - [Transaction signature](#transaction-signature)
  - [SHA256](#sha256)

## Glossary

- **L1**: layer-1 blockchain (Ethereum)
- **Rollup**: layer-2 blockchain (zkSync)
- **Owner**: a user who controls some assets in L2.
- **Operator**: entity operating the Rollup.
- **Eventually**: happening within finite time.
- **Assets in rollup**: assets in L2 smart contract controlled by owners.
- **Rollup key**: owner's private key used to control deposited assets.
- **Rescue signature**: the result of signing the owner's message, using his private key, used in Rollup internal
  transactions.

## Design

### Overview

zkSync implements a ZK rollup protocol (in short "rollup" below) for ETH and ERC20 fungible token transfers.

General rollup workflow is as follows:

- Users can become owners in rollup by depositing assets from L1 or receiving a transfer from other owners.
- Owners can transfer assets to each other.
- Owners can withdraw assets under their control to an L1 address.

Rollup operation requires the assistance of an operator, who rolls transactions together, computes a zero-knowledge
proof of the correct state transition, and affects the state transition by interacting with the rollup contract.

### Assumptions

Cryptography assumptions:

- DLP is unbroken.
- Rescue hash and sha256 are collision-resistant.
- ZKP scheme used in the construction is secure (subject to a separate formal proof).

L1 blockchain assumptions:

- L1 protocol is secure.
- L1 is eventually censorship-resistant: a sufficiently highly priced L1 tx will be mined in a block within finite time.
- Owners have access to the full L1 archive (can at any time retrieve all block bodies of the L1 chain).

Operational assumptions:

- Rollup key is controlled by the owner and not compromised at all times.

### Protocol invariant claims

- [ ] 1. Continuous ownership: assets deposited in rollup are immediately under control of the specified, unique owner.

- [ ] 2. Control: assets in rollup can not be transferred (change owner), change in value, disappear or be moved out of
     rollup, unless the owner initiates a corresponding action.

- [ ] 3. Eventual retrievability: assets in rollup controlled by an owner can eventually be withdrawn to the L1 address
     of the owner's choice, without the cooperation of any other party except L1 miners.

This includes, in particular, the following claims:

- [ ] 3.1. Double-spends in Rollup are not possible.
- [ ] 3.2. Rollup always operates under full reserve: the total supply of each asset type in rollup is equal to the sum
      of all its deposited amounts minus sum of all its withdrawn amounts.
- [ ] 3.3. Root hash is always correct and L2 state can not be corrupted
- [ ] 3.4. A state can always be restored from the calldata published to L1
- [ ] 3.5. The smart contract can not be locked out

## Data format

### Data types

<!-- prettier-ignore -->
|Type|Byte len|Encoding|Comment|
|--|--|--|--|
|AccountId|4|BE integer|Incremented number of accounts in Rollup. New account will have the next free id. Max value is 2^32 - 1 = 4.294967295 × 10^9|
|TokenId|2|BE integer|Incremented number of tokens in Rollup, max value is 65535|
|PackedTxAmount|5|[Parameters](#amount-packing)|Packed transactions amounts are represented with 40 bit (5 byte) values, encoded as mantissa × 10^exponent where mantissa is represented with 35 bits, exponent is represented with 5 bits. This gives a range from 0 to 34359738368 × 10^31, providing 10 full decimal digit precision.|
|PackedFee|2|[Parameters](#amount-packing)|Packed fees must be represented with 2 bytes: 5 bit for exponent, 11 bit for mantissa.|
|StateAmount|16|BE integer|State amount is represented as uint128 with a range from 0 to ~3.4 × 10^38. It allows to represent up to 3.4 × 10^20 "units" if standard Ethereum's 18 decimal symbols are used. This should be a sufficient range.|
|Nonce|4|BE integer|Nonce is the total number of executed transactions of the account. In order to apply the update of this state, it is necessary to indicate the current account nonce in the corresponding transaction, after which it will be automatically incremented. If you specify the wrong nonce, the changes will not occur.|
|RollupPubkeyHash|20|LE integer|To make a public key hash from a Rollup public key apply [Rescue hash function](#rescue-hash) to the `[x,y]` points of the key and then take the last 20 bytes of the result.|
|EthAddress|20|LE integer|To make an Ethereum address from the Etherum's public key, all we need to do is to apply Keccak-256 hash function to the key and then take the last 20 bytes of the result.|
|PackedRollupPubkey|32|LE integer|A Rollup public key is the first 32 bytes of a Rollup public key |
|TxHash|32|LE integer|To get hash for transaction apply [SHA256 function](#sha256) to concatenated bytes of [transaction fields](#zk-sync-operations)|
|Signature|64|LE integer|Read [transaction signature](#transaction-signature)|
|BlockNumber|4|BE integer|Incremented number of Rollup blocks, max number is 4294967295|
|RootHash|32|LE integer|[Merkle tree root hash](#sparse-merkle-tree)|
|Timestamp|64|BE integer| unix timestamp |

### Amount packing

Amounts and fees are compressed in zkSync using simple
[fundamentals of floating point arithmetic](https://en.wikipedia.org/wiki/Floating-point_arithmetic).

A floating-point number has the following parts: a mantissa, a radix, and an exponent. The mantissa (always non-negative
in our case) holds the significant digits of the floating-point number. The exponent indicates the power of the radix
that the mantissa and sign should be multiplied by. The components are combined as follows to get the floating-point
value:

```
sign * mantissa * (radix ^ exponent)
```

Mantissa and exponent parameters used in zkSync:

| Type           | Exponent bit width | Mantissa bit width | Radix |
| -------------- | ------------------ | ------------------ | ----- |
| PackedTxAmount | 5                  | 35                 | 10    |
| PackedFee      | 5                  | 11                 | 10    |

### State Merkle Tree

Accounts and Balances trees representation:

![Merkle Tree](https://i.imgur.com/itXl2UV.png)

Legend:

- Ha is account tree height. (24)
- Hb is balance tree height. (8)

We have directly one main `Accounts tree` and its leaves `Accounts` also have subtrees `Balances tree` with their own
`Balances` leaves.

#### Leaf hash

**The leaf hash** is the [rescue hash](#rescue-hash) of its bit string representation described below. To get bit string
representation each filed is encoded as bits starting from least significant (LE order for bits) and concatenated in the
order they are present in the structure.

#### Account leaf

Each account is inserted into the Accounts tree as a leaf with the least free id (`AccountId`).

| Field           | Type             |
| --------------- | ---------------- |
| nonce           | Nonce            |
| pubkey_hash     | RollupPubkeyHash |
| address         | EthAddress       |
| state_tree_root | RootHash         |

`state_tree_root` is combined using [Rescue hash](#rescue-hash) of the root hash of the `balance_tree_root`(as a field
element) and zero field element reserved for future subtree root hash and then padded to 256 bits (by adding 0 bits in
LE bit representation)

An empty leaf contains: `state_tree_root` computed using empty balances subtree. Empty balances subtree of the account
has zero in each leaf.

#### Balance leaf

| Field | Type        |
| ----- | ----------- |
| value | StateAmount |

And empty leaf contains `value` equal to zero.

### zkSync block pub data format

Rollup block pub data consists of [Rollup operations](#zksync-operations) pub data sequence. The maximum block size is a
constant value. If the size of the operations included in the block is not enough to fill it completely, the remainder
will be filled with empty [Noop](#1-noop-operation) operations.

## zkSync operations

zkSync operations are divided into Rollup transactions (initiated inside Rollup by a Rollup account) and Priority
operations (initiated on the mainchain by an Ethereum account).

Rollup transactions:

- Noop
- Transfer
- Transfer to new
- Withdraw (Partial exit)
- Change pubkey
- Forced Exit
- MintNFT
- WithdrawNFT
- Swap

Priority operations:

- Deposit
- Full exit

Full list: <https://docs.google.com/spreadsheets/d/1ejK1MJfVehcwjgjVDFD3E2k1EZ7auqbG_y0DKidS9nA/edit#gid=0>

Legend:

- Transaction: what users can submit to the operator directly.
- Priority operation: what users can submit to the zkSync smart contract.
- Rollup operation: part of the rollup block representing `Transaction` or `Priority operation`.
- Onchain operation: what the operator can put into the rollup block pubdata (operation pubdata).
- Node implementation: node model that describes an operation.
- Circuit implementation: circuit model that describes the operation and its witness.
- Chunk: the dimension of the operation. Each chunk has its own part of the public data (10 bytes) given through
  witnesses.
- Significant bytes: how many bytes, of all bytes occupied by the operation, are significant (including operation
  number).
- Hash: the result of SHA-256 function with operation's pubdata as input. Used for operation identification.

### 0. Rollup operation lifecycle

1. User creates a `Transaction` or a `Priority operation`.
2. After processing this request, operator creates a `Rollup operation` and adds it to the block.
3. Once the block is complete, operator submits it to the zkSync smart contract as a block commitment. Part of the logic
   of some `Rollup operations` is checked by the smart contract.
4. The proof for the block is submitted to the zkSync smart contract as the block verification. If the verification
   succeeds, the new state is considered finalized.

#### Definitions used in circuit description

Critical circuit logic is described below in a python-like pseudocode for simplicity of comprehension. It describes
invariants that should be preserved after rollup operation execution and account tree updates that should be performed
if this invariants are true.

There are two main invariant functions: `tree_invariants` and `pubdata_invariants`. Each predicate in these functions
should evaluate to true.

Functions and data structures used in this pseudocode language:

```
# Returns account in the account tree given its id; it always returns something, since account tree is filled by empty account by default.
get_account_tree(account_id) -> Account

# Unpacks packed amount or fee; these functions implicitly enforce the invariant that amount and fee is packable
unpack_amount(packed_amount) -> StateAmount
unpack_fee(packed_fee) -> StateAmount

# Checks if account is empty: address, pubkey_hash, nonce and all balances are 0.
is_account_empty(Account) -> bool

# Returns signer for the given transaction
recover_signer_pubkey_hash(tx) -> RollupPubkeyHash

# Account data structure
Account.balance[token_id] -> StateAmount # returns account balance of the given token
Account.nonce -> Nonce # Nonce of the account
Account.address -> EthAddress # Address of the account
Account.pubkey_hash -> RollupPubkeyHash # Hash of the public key set for the account

# Constants
MAX_TOKENS = 2**32 # maximum number of tokens in the Rollup(including "ETH" token)
MAX_FUNGIBLE_TOKENS = 2**16 # maximum number of fungible tokens in the Rollup(including "ETH" token)
MAX_NONCE = 2**32 # max possible nonce
```

### 1. Noop operation

#### Description

No effects.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 1      | 1                 |

##### Structure

| Field  | Byte len | Value/type | Description    |
| ------ | -------- | ---------- | -------------- |
| Opcode | 1        | `0x00`     | Operation code |

##### Example

```
000000000000000000
```

#### User transaction

No user transaction.

#### Effects

No effects. This operation is used for padding the block with zero bytes (cheap in calldata) to the full block capacity.

### 2. Transfer

#### Description

Transfers funds between Rollup accounts.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 2      | 20                |

##### Structure

| Field         | Byte len | Value/type     | Description                                                                         |
| ------------- | -------- | -------------- | ----------------------------------------------------------------------------------- |
| opcode        | 1        | `0x05`         | Operation code                                                                      |
| from_account  | 4        | AccountId      | Unique identifier of the rollup account from which funds will be withdrawn (sender) |
| token         | 4        | TokenId        | Unique token identifier in the rollup                                               |
| to_account    | 4        | AccountId      | Unique identifier of the rollup account that will receive the funds (recipient)     |
| packed_amount | 5        | PackedTxAmount | Packed amount of funds sent                                                         |
| packed_fee    | 2        | PackedFee      | Packed amount of fee paid                                                           |

##### Example

```
050000000400000002000000030000001ad30012
```

Reads as: transfer from account #4 token #2 to account #3 amount in packed representation 0x0000001ad3 for fee in packed
representation 0x0012.

#### User transaction

##### Structure

| Field       | Value/type     | Description                                                                      |
| ----------- | -------------- | -------------------------------------------------------------------------------- |
| type        | `0xfa`         | Operation code                                                                   |
| account_id  | AccountId      | Unique id of the sender rollup account in the state tree                         |
| from        | ETHAddress     | Unique address of the rollup account from which funds will be withdrawn (sender) |
| to          | ETHAddress     | Unique address of the rollup account that will receive the funds (recipient)     |
| token       | TokenId        | Unique token identifier in the rollup                                            |
| amount      | PackedTxAmount | Amount of funds sent                                                             |
| fee         | PackedFee      | Amount of fee paid                                                               |
| nonce       | Nonce          | A one-time code that specifies the order of transactions                         |
| valid_from  | Timestamp      | Unix timestamp from which the block with this transaction can be processed       |
| valid_until | Timestamp      | Unix timestamp until which the block with this transaction can be processed      |
| signature   | Signature      | [Signature](#transaction-singature) of previous fields, see the spec below       |

##### Example

User transaction representation. (NOTE: tx bytecode differs slightly from this representation due to data packing, see
the spec below).

```json
{
  "type": "Transfer",
  "accountId": 2061,
  "from": "0x1f04204dba8e9e8bf90f5889fe4bdc0f37265dbb",
  "to": "0x05e3066450dfcd4ee9ca4f2039d58883631f0460",
  "token": 1023,
  "amount": "12340000000000",
  "fee": "56700000000",
  "nonce": 784793056,
  "validFrom": 0,
  "validUntil": 18446744073709551615,
  "signature": {
    "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
    "signature": "817c866e71a0b3e6d412ac56524557d368c11332db93554693787e89c9813310adeda68314fc833a4f73323eca00e2cc774e78db88921dc230db7dae691fe500"
  }
}
```

Signed transaction representation.

```
Signer:
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signed transaction fields:
type: 0xfa
version: 0x01
accountId: 0x0000080d
from: 0x1f04204dba8e9e8bf90f5889fe4bdc0f37265dbb
to: 0x05e3066450dfcd4ee9ca4f2039d58883631f0460
token: 0x000003ff
amount: 0x5bf0aea003
fee: 0x46e8
nonce: 0x2ec6fde0
time_range: 0x0000000000000000ffffffffffffffff
Signed bytes: 0xfa010000080d1f04204dba8e9e8bf90f5889fe4bdc0f37265dbb05e3066450dfcd4ee9ca4f2039d58883631f0460000003ff5bf0aea00346e82ec6fde00000000000000000ffffffffffffffff
```

#### Rollup operation

##### Structure

| Field           | Value/type | Description                                                 |
| --------------- | ---------- | ----------------------------------------------------------- |
| tx              | TransferTx | Signed transfer transaction defined above                   |
| from_account_id | AccountId  | Unique id of the sender rollup account in the state tree    |
| to_account_id   | AccountId  | Unique id of the recipient rollup account in the state tree |

#### Circuit constraints

```python
# TransferOp - Rollup operation described above
# Block - block where this Rollup operation is executed
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

from_account = get_account_tree(TransferOp.from_account_id)
to_account = get_account_tree(TransferOp.to_account_id)
fee_account = get_account_tree(Block.fee_account)

amount = unpack_amount(TransferOp.tx.packed_amount)
fee = unpack_fee(TransferOp.tx.packed_fee)

def tree_invariants():
    TransferOp.token < MAX_TOKENS

    from_account.id == TransferOp.tx.from_account_id;
    from_account.nonce == TransferOp.tx.nonce
    from_account.nonce < MAX_NONCE
    from_account.balance(TransferOp.tx.token) >= (amount + fee)
    from_account.pubkey_hash == recover_signer_pubkey_hash(TransferOp.tx)
    from_account.address == TransferOp.tx.from_address

    to_account.address == TransferOp.tx.to_address

def tree_updates():
    from_account.balance[TransferOp.tx.token] -= (amount + fee)
    from_account.nonce += 1

    to_acccount.balance[TransferOp.tx.token] += amount

    fee_account.balance[TransferOp.tx.token] += fee

def pubdata_invariants():
    OnchainOp.opcode == 0x05
    OnchainOp.from_account == TransferOp.from_account_id
    OnchainOp.token == TransferOp.tx.token
    OnchainOp.to_account == TransferOp.to_account_id
    OnchainOp.packed_amount == TransferOp.tx.packed_amount
    OnchainOp.packed_fee == TransferOp.tx.packed_fee
```

### 3. Transfer to new

#### Description

Transfers funds from Rollup account to a new Rollup account (dynamically assigns a free account number to a Rollup
address). So, the "account creation" will be performed first, that is, the correspondence RollupAddress - AccountId is
assigned. And then the usual funds' Transfer between Rollup accounts will occur.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 6      | 40                |

##### Structure

| Field         | Byte len | Value/type     | Description                                                                         |
| ------------- | -------- | -------------- | ----------------------------------------------------------------------------------- |
| opcode        | 1        | `0x02`         | Operation code                                                                      |
| from_account  | 4        | AccountId      | Unique identifier of the rollup account from which funds will be withdrawn (sender) |
| token         | 4        | TokenId        | Unique token identifier in the rollup                                               |
| packed_amount | 5        | PackedTxAmount | Packed amount of funds sent                                                         |
| to_address    | 20       | ETHAddress     | The address of the rollup account that will receive the funds (recipient)           |
| to_account    | 4        | AccountId      | Unique identifier of the rollup account that will receive the funds (recipient)     |
| packed_fee    | 2        | PackedFee      | Packed amount of fee paid                                                           |

##### Example

```
0200000004000000020000001ad3080910111213141516171819202122233425262800000003001200000000
```

Reads as: transfer from account #4 token #2 amount in packed representation 0x0000001ad3 to account with address
0x0809101112131415161718192021222334252628 and id #3 for fee in packed representation 0x0012.

#### User transaction

Same as [Transfer](#2-transfer)

#### Rollup operation

##### Structure

| Field           | Value/type | Description                                                 |
| --------------- | ---------- | ----------------------------------------------------------- |
| tx              | TransferTx | Signed transfer transaction defined above                   |
| from_account_id | AccountId  | Unique id of the sender rollup account in the state tree    |
| to_account_id   | AccountId  | Unique id of the recipient rollup account in the state tree |

#### Circuit constraints

```python
# TransferToNewOp - Rollup operation described above
# Block - block where this Rollup operation is executed
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

from_account = get_account_tree(TransferToNewOp.from_account_id)
to_account = get_account_tree(TransferToNewOp.to_account_id)
fee_account = get_account_tree(Block.fee_account)

amount = unpack_amount(TransferToNewOp.tx.packed_amount)
fee = unpack_fee(TransferToNewOp.tx.packed_fee)

def tree_invariants():
    TransferToNewOp.token < MAX_TOKENS

    from_account.id == TransferToNewOp.tx.from_account_id;
    from_account.nonce == TransferToNewOp.tx.nonce
    from_account.nonce < MAX_NONCE
    from_account.balance[TransferToNewOp.tx.token] >= (amount + fee)
    from_account.pubkey_hash == recover_signer_pubkey_hash(TransferToNewOp.tx)
    from_account.address == TransferToNewOp.tx.from_address

    is_account_empty(to_account) == True

def tree_updates():
    from_account.balance[TransferOp.tx.token] -= (amount + fee)
    from_account.nonce += 1

    to_account.address = TransferToNewOp.to_address
    to_account.balance[TransferOp.tx.token] += amount

    fee_account.balance[TransferOp.tx.token] += fee

def pubdata_invariants():
    OnchainOp.opcode == 0x02
    OnchainOp.from_account == TransferToNewOp.from_account_id
    OnchainOp.token == TransferToNewOp.tx.token
    OnchainOp.packed_amount == TransferToNewOp.tx.packed_amount
    OnchainOp.to_address == TransferToNewOp.tx.to_address
    OnchainOp.to_account == TransferToNewOp.to_account_id
    OnchainOp.packed_fee == TransferToNewOp.tx.packed_fee
```

### 4. Withdraw (Partial Exit)

#### Description

Withdraws funds from Rollup account to appropriate balance of the indicated Ethereum address.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 6      | 47                |

##### Structure

| Field        | Byte len | Value/type  | Description                                                                                |
| ------------ | -------- | ----------- | ------------------------------------------------------------------------------------------ |
| opcode       | 1        | `0x03`      | Operation code                                                                             |
| from_account | 4        | AccountId   | Unique identifier of the rollup account from which funds will be withdrawn (sender)        |
| token        | 4        | TokenId     | Unique token identifier in the rollup                                                      |
| full_amount  | 16       | StateAmount | Full amount of funds sent                                                                  |
| packed_fee   | 2        | PackedFee   | Packed amount of fee paid                                                                  |
| to_address   | 20       | EthAddress  | The address of Ethereum account, to the balance of which funds will be accrued (recipient) |

##### Example

```
030000000400000002000000000000000002c68af0bb1400000012080910111213141516171819202122233425262800000000
```

Reads as: transfer from account #4 token #2 amount 0x000000000000000002c68af0bb140000 for fee packed in representation
0x0012 to ethereum account with address 0x0809101112131415161718192021222334252628.

#### User transaction

##### Structure

| Field        | Value/type  | Description                                                                                   |
| ------------ | ----------- | --------------------------------------------------------------------------------------------- |
| type         | `0xfc`      | Operation code                                                                                |
| account_id   | AccountId   | Unique id of the sender rollup account in the state tree                                      |
| from_address | ETHAddress  | Unique address of the rollup account from which funds will be withdrawn (sender)              |
| to_address   | EthAddress  | The address of Ethereum account, to the balance of which the funds will be accrued(recipient) |
| token        | TokenId     | Unique token identifier in the rollup                                                         |
| amount       | StateAmount | Full amount of funds sent                                                                     |
| fee          | PackedFee   | Packed amount of fee paid                                                                     |
| nonce        | Nonce       | A one-time code that specifies the order of transactions                                      |
| valid_from   | Timestamp   | Unix timestamp from which the block with this transaction can be processed                    |
| valid_until  | Timestamp   | Unix timestamp until which the block with this transaction can be processed                   |
| signature    | Signature   | [Signature](#transaction-singature) of previous fields, see the spec below                    |

##### Example

User transaction representation. (NOTE: tx bytecode differs slightly from this representation due to data packing, see
the spec below)..

```json
{
  "type": "Withdraw",
  "accountId": 4118,
  "from": "0x041f3b8db956854839d7434f3e53c7141a236b16",
  "to": "0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513",
  "token": 1023,
  "amount": "12340000000000",
  "fee": "56700000000",
  "nonce": 352676723,
  "signature": {
    "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
    "signature": "e29781125bdd3c1d0a4e863e7d364979dd3e3a36c2cd31fcf18d3853ae417201632c5c6ce4ac24da6a3ca99ed7e2005a74e6d6e2a3378d4102d68efb02d28d03"
  },
  "fast": false,
  "validFrom": 0,
  "validUntil": 18446744073709551615
}
```

Signed transaction representation.

```
Signer:
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signed transaction fields:
type: 0xfc
version: 0x01
accountId: 0x00001016
from: 0x041f3b8db956854839d7434f3e53c7141a236b16
to: 0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513
token: 0x000003ff
fullAmount: 0x000000000000000000000b3921510800
fee: 0x46e8
nonce: 0x15056b73
time_range: 0x0000000000000000ffffffffffffffff
Signed bytes: 0xfc0100001016041f3b8db956854839d7434f3e53c7141a236b16dc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513000003ff000000000000000000000b392151080046e815056b730000000000000000ffffffffffffffff
```

#### Rollup operation

##### Structure

| Field | Value/type | Description                               |
| ----- | ---------- | ----------------------------------------- |
| tx    | WithdrawTx | Signed withdraw transaction defined above |

#### Circuit constraints

```python
# WithdrawOp - Rollup operation described above
# Block - block where this Rollup operation is executed
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

account = get_tree_account(WithdrawOp.tx.account_id)
fee_account = get_tree_account(Block.fee_account)

fee = unpack_fee(WithdrawOp.tx.packed_fee)

def tree_invariants():
    WithdrawOp.token < MAX_FUNGIBLE_TOKENS

    account.nonce == WithdrawOp.nonce
    account.nonce < MAX_NONCE
    account.balance[WithdrawOp.tx.token] >= (amount + fee)
    account.pubkey_hash == recover_signer_pubkey_hash(WithdrawOp.tx)


def tree_updates():
    account.balance[WithdrawOp.tx.token] -= (amount + fee)
    account.nonce += 1

    fee_account.balance[WithdrawOp.token] += fee

def pubdata_invariants():
    OnchainOp.opcode == 0x03
    OnchainOp.from_account == WithdrawOp.tx.account_id
    OnchainOp.token == WithdrawOp.tx.token
    OnchainOp.full_amount == WithdrawOp.tx.amount
    OnchainOp.packed_fee == WithdrawOp.tx.packed_fee
    OnchainOp.to_address == WithdrawOp.tx.to_address
```

### 5. Withdraw NFT

#### Description

Withdraws NFT from Rollup account to appropriate ethereum account.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 10     | 95                |

##### Structure

| Field           | Byte len | Value/type | Description                                                                                |
| --------------- | -------- | ---------- | ------------------------------------------------------------------------------------------ |
| opcode          | 1        | `0x0a`     | Operation code                                                                             |
| from_account    | 4        | AccountId  | Unique identifier of the rollup account from which funds will be withdrawn (sender)        |
| creator_account | 4        | AccountId  | Unique identifier of the rollup account which create the NFT (creator)                     |
| creator_address | 20       | EthAddress | The address of Ethereum account, to the balance of which create the NFT (creator)          |
| serial_id       | 4        | Int        | Special id for NFT for enforcing uniqueness                                                |
| content_hash    | 32       | H256       | Content hash of NFT                                                                        |
| to_address      | 20       | EthAddress | The address of Ethereum account, to the balance of which funds will be accrued (recipient) |
| token           | 4        | TokenId    | Unique token identifier in the rollup                                                      |
| fee_token       | 4        | TokenId    | Token for paying fees                                                                      |
| packed_fee      | 2        | PackedFee  | Packed amount of fee paid                                                                  |

##### Example

```
0a0000002a0000002b21abaed8712072e918632259780e587698ef58da00000000000000000000000000000000000000000000000000000000000000000000000021abaed8712072e918632259780e587698ef58da000100000000002a05400000000000
```

Reads as: Withdraw NFT from account #4 token #2 for fee packed in representation and paying fee in token 0x0012 to
ethereum account with address 0x0809101112131415161718192021222334252628.

#### User transaction

##### Structure

| Field        | Value/type | Description                                                                                   |
| ------------ | ---------- | --------------------------------------------------------------------------------------------- |
| type         | `0xf5`     | Operation code                                                                                |
| account_id   | AccountId  | Unique id of the sender rollup account in the state tree                                      |
| from_address | ETHAddress | Unique address of the rollup account from which funds will be withdrawn (sender)              |
| to_address   | EthAddress | The address of Ethereum account, to the balance of which the funds will be accrued(recipient) |
| token        | TokenId    | Unique token identifier in the rollup                                                         |
| fee_token    | TokenId    | Fee token identifier in the rollup                                                            |
| fee          | PackedFee  | Packed amount of fee paid                                                                     |
| nonce        | Nonce      | A one-time code that specifies the order of transactions                                      |
| valid_from   | Timestamp  | Unix timestamp from which the block with this transaction can be processed                    |
| valid_until  | Timestamp  | Unix timestamp until which the block with this transaction can be processed                   |
| signature    | Signature  | [Signature](#transaction-singature) of previous fields, see the spec below                    |

##### Example

User transaction representation. (NOTE: tx bytecode differs slightly from this representation due to data packing, see
the spec below)..

```json
{
  "type": "WithdrawNFT",
  "accountId": 4118,
  "from": "0x041f3b8db956854839d7434f3e53c7141a236b16",
  "to": "0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513",
  "token": 274474656,
  "feeToken": 1023,
  "fee": "12340000000000",
  "nonce": 3924379879,
  "signature": {
    "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
    "signature": "55aea80f1359cbedf68f86509a51ab721c7656263b221f8afa38a6c794ba181d119396c3d82e93feb61d2a5bef44069a3c1a7086206d7fab923abcc16e30b501"
  },
  "fast": false,
  "validFrom": 0,
  "validUntil": 18446744073709551615
}
```

Signed transaction representation.

```
Signer:
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signed transaction fields:
type: 0xf5
version: 0x01
accountId: 0x00001016
from: 0x041f3b8db956854839d7434f3e53c7141a236b16
to: 0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513
token: 0x105c26a0
fee_token: 0x000003ff
fee: 0x9a4a
nonce: 0xe9e948e7
time_range: 0x0000000000000000ffffffffffffffff
Signed bytes: 0xf50100001016041f3b8db956854839d7434f3e53c7141a236b16dc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513105c26a0000003ff9a4ae9e948e70000000000000000ffffffffffffffff
```

#### Rollup operation

##### Structure

| Field | Value/type    | Description                               |
| ----- | ------------- | ----------------------------------------- |
| tx    | WithdrawNFTTx | Signed withdraw transaction defined above |

#### Circuit constraints

```python
# WithdrawNFTOp - Rollup operation described above
# Block - block where this Rollup operation is executed
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

account = get_tree_account(WithdrawNFTOp.tx.account_id)
fee_account = get_tree_account(Block.fee_account)

fee = unpack_fee(WithdrawNFTOp.tx.packed_fee)

def tree_invariants():
    WithdrawNFTOp.token < MAX_TOKENS
    WithdrawNFTOp.token > MAX_FUNGIBLE_TOKENS
    WithdrawNFTOp.fee_token < MAX_FUNGIBLE_TOKENS

    account.nonce == WithdrawNFTOp.nonce
    account.nonce < MAX_NONCE
    account.balance[WithdrawNFTOp.tx.token] == 1
    account.balance[WithdrawNFTOp.tx.fee_token] >= fee
    account.pubkey_hash == recover_signer_pubkey_hash(WithdrawNFTOp.tx)


def tree_updates():
    account.balance[WithdrawNFTOp.tx.token] = 0
    account.balance[WithdrawNFTOp.tx.fee_token] -= fee
    account.nonce += 1

    fee_account.balance[WithdrawNFTOp.token] += fee

def pubdata_invariants():
    OnchainOp.opcode == 0x0a
    OnchainOp.from_account == WithdrawNFTOp.tx.account_id
    OnchainOp.token == WithdrawNFTOp.tx.token
    OnchainOp.fee_token == WithdrawNFTOp.tx.fee_token
    OnchainOp.packed_fee == WithdrawNFTOp.tx.packed_fee
    OnchainOp.to_address == WithdrawNFTOp.tx.to_address
```

### 6. Mint NFT

#### Description

Mints an NFT token inside Rollup

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 5      | 47                |

##### Structure

| Field           | Byte len | Value/type  | Description                                                     |
| --------------- | -------- | ----------- | --------------------------------------------------------------- |
| opcode          | 1        | `0x09`      | Operation code                                                  |
| creator_account | 4        | AccountId   | Unique identifier of the rollup account will mint nft (creator) |
| recipient       | 4        | AccountId   | Recipient of NFT                                                |
| content_hash    | 32       | ContentHash | Content for NFT                                                 |
| fee_token       | 4        | TokenId     | Fee for paying fee                                              |
| packed_fee      | 2        | PackedFee   | Packed amount of fee paid                                       |

##### Example

```
090000000a0000000b0000000000000000000000000000000000000000000000000000000000000000000000000140000000
```

Reads as: Mint NFT from account to recipient with content hash and pay packed fee in fee_token

#### User transaction

##### Structure

| Field        | Value/type | Description                                                                                   |
| ------------ | ---------- | --------------------------------------------------------------------------------------------- |
| type         | `0xf6`     | Operation code                                                                                |
| account_id   | AccountId  | Unique id of the sender rollup account in the state tree                                      |
| from_address | ETHAddress | Unique address of the rollup account from which funds will be withdrawn (sender)              |
| to_address   | EthAddress | The address of Ethereum account, to the balance of which the funds will be accrued(recipient) |
| token        | TokenId    | Unique token identifier in the rollup                                                         |
| fee_token    | TokenId    | Fee token identifier in the rollup                                                            |
| fee          | PackedFee  | Packed amount of fee paid                                                                     |
| nonce        | Nonce      | A one-time code that specifies the order of transactions                                      |
| signature    | Signature  | [Signature](#transaction-singature) of previous fields, see the spec below                    |

##### Example

User transaction representation. (NOTE: tx bytecode differs slightly from this representation due to data packing, see
the spec below)..

```json
{
  "type": "MintNFT",
  "creatorId": 4118,
  "creatorAddress": "0x041f3b8db956854839d7434f3e53c7141a236b16",
  "contentHash": "0x0d185e587f0a80e93cd0f311cf9c7a0071ae63d088cfc810de55c9f6d9fb4bcc",
  "recipient": "0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513",
  "fee": "12340000000000",
  "feeToken": 1023,
  "nonce": 352676723,
  "signature": {
    "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
    "signature": "59aff7f0f3dff5c6b0efc1f919599e62214bb99933438b74acb96076598b41ae4976b68328492978b349e907aebfc9dce2b1298debda589686517780afae7203"
  }
}
```

Signed transaction representation.

```
Signer:
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signed transaction fields:
type: 0xf6
version: 0x01
creatorId: 0x00001016
creatorAddress: 0x041f3b8db956854839d7434f3e53c7141a236b16
contentHash: 0x0d185e587f0a80e93cd0f311cf9c7a0071ae63d088cfc810de55c9f6d9fb4bcc
recipient: 0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513
fee_token: 0x000003ff
fee: 0x9a4a
nonce: 0x15056b73

Signed bytes: 0xf60100001016041f3b8db956854839d7434f3e53c7141a236b160d185e587f0a80e93cd0f311cf9c7a0071ae63d088cfc810de55c9f6d9fb4bccdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513000003ff9a4a15056b73
```

#### Rollup operation

##### Structure

| Field | Value/type | Description                               |
| ----- | ---------- | ----------------------------------------- |
| tx    | MintNFTTx  | Signed mint nft transaction defined above |

#### Circuit constraints

```python
# MintNFTOp - Rollup operation described above
# Block - block where this Rollup operation is executed
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

creator_account = get_tree_account(MintNFTOp.tx.creator_id)
recipient_account = get_tree_account(MintNFTOp.tx.recipient)
fee_account = get_tree_account(Block.fee_account)

fee = unpack_fee(MintNFTOp.tx.packed_fee)

def tree_invariants():
    MintNFTOp.fee_token < MAX_FUNGIBLE_TOKENS

    creator_account.nonce == MintNFTOp.nonce
    creator_account.nonce < MAX_NONCE
    creator_account.balance[MintNFTOp.tx.fee_token] >= fee
    creator_account.pubkey_hash == recover_signer_pubkey_hash(MintNFTOp.tx)


def tree_updates():
    creator_account.balance[MintNFTOp.tx.fee_token] -= fee
    creator_account.balance[SPECIAL_NFT_TOKEN] += 1
    special_nft_account.balance[SPECIAL_NFT_TOKEN] += 1
    creator_account.nonce += 1
    recipient_account[minted_token] = 1
    fee_account.balance[MintNFTOp.token] += fee

def pubdata_invariants():
    OnchainOp.opcode == 0x09
    OnchainOp.creator_account == MintNFTOp.tx.account_id
    OnchainOp.recipient == MintNFTOp.tx.recipient
    OnchainOp.fee_token == MintNFTOp.tx.fee_token
    OnchainOp.packed_fee == MintNFTOp.tx.packed_fee
```

### 7. Deposit

#### Description

Deposits funds from Ethereum account to the specified Rollup account. Deposit starts as priority operation - user calls
contract method [`depositEth`](#deposit-ether) to deposit ethereum, or [`depositERC20`](#deposit-erc-20-token) to
deposit ERC-20 tokens. After that operator includes this operation in a block. In the account tree, the new account will
be created if needed.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 6      | 45                |

##### Structure

| Field       | Byte len | Value/type  | Description                                                                     |
| ----------- | -------- | ----------- | ------------------------------------------------------------------------------- |
| opcode      | 1        | `0xfe`      | Operation code                                                                  |
| to_account  | 4        | AccountId   | Unique identifier of the rollup account that will receive the funds (recipient) |
| token       | 4        | TokenId     | Unique token identifier in the rollup                                           |
| full_amount | 16       | StateAmount | Full amount of funds sent                                                       |
| to_address  | 20       | ETHAddress  | The address of the rollup account that will receive the funds (recipient)       |

##### Example

```
fe010000000400000002000000000000000002c68af0bb1400000809101112131415161718192021222334252628000000000000
```

Reads as: deposit to account #4 token #2 amount 0x000000000000000002c68af0bb140000, account will have address
0x0809101112131415161718192021222334252628.

#### Priority operation

##### Structure

| Field       | Byte len | Value/type  | Description                                                               |
| ----------- | -------- | ----------- | ------------------------------------------------------------------------- |
| token       | 4        | TokenId     | Unique token identifier in the rollup                                     |
| full_amount | 16       | StateAmount | Full amount of funds sent                                                 |
| to_address  | 20       | ETHAddress  | The address of the rollup account that will receive the funds (recipient) |

#### Rollup operation

##### Structure

| Field         | Value/type        | Description                                                                     |
| ------------- | ----------------- | ------------------------------------------------------------------------------- |
| op            | DepositPriorityOp | Priority operation defined above                                                |
| to_account_id | AccountId         | Unique identifier of the rollup account that will receive the funds (recipient) |

#### Circuit constraints

```python
# DepositOp - Rollup operation described above
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

account = get_account_tree(DepositOp.to_account_id)

def tree_invariants():
    DepositOp.token < MAX_FUNGIBLE_TOKENS

    is_account_empty(account) == True or account.address == DepositOp.op.to_address


def tree_updates():
    if is_account_empty(account):
        account.address = DepositOp.op.to_address

    account.balance[DepositOp.op.token] += DepositOp.op.full_amount

def pubdata_invariants():
    OnchainOp.opcode == 0x01
    OnchainOp.to_account == DepositOp.to_account_id
    OnchainOp.token == DepositOp.op.token
    OnchainOp.full_amount == DepositOp.op.amount
    OnchainOp.to_address == DepositOp.op.to_address
```

#### Censorship by the operator

It is possible that the operator for some reason does not include this operation in the block. Then, through the number
of ethereum blocks set on the smart contract, the exodus mode will be launched. It will allow recipient account (i.e.
with `msg.sender == Deposit.to`) to withdraw funds from zkSync contract.

### 8. Full exit

#### Description

The user can request this operation to withdraw funds if he thinks that his transactions are censored by validators.

It starts as a priority operation - user calls contract method `fullExit`. After that operator includes this operation
in a block.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 11     | 85                |

##### Structure

| Field                         | Byte len | Value/type  | Description                                                                                 |
| ----------------------------- | -------- | ----------- | ------------------------------------------------------------------------------------------- |
| opcode                        | 1        | `0xf9`      | Operation code                                                                              |
| account_id                    | 4        | AccountId   | Unique identifier of the rollup account from which funds will be withdrawn (sender)         |
| owner                         | 20       | EthAddress  | The address of the fund owner account. Funds will be accrued to the balance of this address |
| token                         | 4        | TokenId     | Unique token identifier in the rollup                                                       |
| full_amount                   | 16       | StateAmount | Full amount of funds that had been withdrawn                                                |
| creator_account_id (Optional) | 4        | AccountId   | Unique identifier of the rollup account from which created nft                              |
| serial_id (Optional)          | 4        | Int         |                                                                                             |
| content_hash (Optional)       | 32       | ContentHash | Content of NFT                                                                              |

##### Example

```
060000002a2a0a81e257a2f5d6ed4f07b81dbda09f107bd0260000002a000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000
```

Reads as: full exit from account #4 with with address 0x0809101112131415161718192021222334252628, token #2, amount is
0x000000000000000002c68af0bb140000.

#### Priority operation

##### Structure

| Field       | Byte len | Value/type | Description                             |
| ----------- | -------- | ---------- | --------------------------------------- |
| account_id  | 4        | AccountId  | Unique identifier of the rollup account |
| eth_address | 20       | ETHAddress | The address of the account              |
| token       | 4        | TokenId    | Unique token identifier in the rollup   |

#### Rollup operation

##### Structure

| Field | Value/type         | Description                      |
| ----- | ------------------ | -------------------------------- |
| op    | FullExitPriorityOp | Priority operation defined above |

#### Circuit constraints

```python
# FullExitOp - Rollup operation described above
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

account = get_account_tree(FullExitOp.op.account_id)
withdrawn_amount = 0

def tree_invariants():
    FullExitOp.op.token < MAX_TOKENS
    account.id == FullExitOp.op.id

def tree_updates():
    if account.address == FullExitOp.op.eth_address:
        withdrawn_amount = account.balance[FullExitOp.op.token]
        account.balance[FullExitOp.op.token] = 0

def pubdata_invariants():
    OnchainOp.opcode == 0x06
    OnchainOp.account_id == FullExitOp.op.account_id
    OnchainOp.owner == FullExitOp.op.eth_address
    OnchainOp.token == FullExitOp.op.token
    OnchainOp.full_amount == withdrawn_amount
```

#### Failure signal

If something went wrong on the server side - full exit operation will be included in a block with 0 (zero) amount in
pubdata.

#### Censorship by the operator

It is possible that the operator for some reason does not include this operation in the block. Then, through the number
of ethereum blocks set on the smart contract, the exodus mode will be launched. After that, a user can submit exit proof
to get her funds. Read more about censorship resistance and exodus mode in special sections.

### 9. Change pubkey

#### Description

Change pubkey - changes public key of the account that is used to authorize transactions. Change pubkey is authorized
with ethereum keys for which address is the same as account address.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 6      | 49                |

##### Structure

| Field           | Byte len | Value/type       | Description                                           |
| --------------- | -------- | ---------------- | ----------------------------------------------------- |
| opcode          | 1        | `0x07`           | Operation code                                        |
| account_id      | 4        | AccountId        | Unique identifier of the rollup account               |
| new_pubkey_hash | 20       | RollupPubkeyHash | Hash of the new rollup public key                     |
| account_address | 20       | ETHAddress       | Address of the account                                |
| nonce           | 4        | Nonce            | Account nonce                                         |
| fee_token       | 4        | TokenId          | Unique token identifier in the rollup used to pay fee |
| packed_fee      | 2        | PackedFee        | Packed amount of fee paid                             |

##### Example

```
0700000411036945fcc11c349c3a300f19cd87cb03c4f2ef03e69588c1f4155dec60da3bf5113e029911ce330000000300010012
```

Reads as: change pubkey, account #4, new pubkey hash 0x11036945fcc11c349c3a300f19cd87cb03c4f2ef, address:
0x03e69588c1f4155dec60da3bf5113e029911ce33, nonce: 3, fee paid in token #0x0001, fee amount is 0x0012.

#### Auth

1. Transaction can be authorized by providing signature of the message
   `pubkey_message(account_id, nonce, new_pubkey_hash, additional_data)` (see definition below). Transaction will be
   verified on the contract.
2. For users that can't sign messages it is possible to authorize this operation by calling `setAuthPubkeyHash` method
   of the smart contract. User should provide new pubkey hash and nonce for this transaction. After this transaction
   succeeded transaction without signature can be sent to operator.
3. For users that want to use smart contract as a wallet and don't want to deploy it unless necessary it is possible to
   authorize change pubkey using CREATE2-change pubkey variant where wallet address is known and is derived from
   `new_pubkey_hash`.

   We check that account address is equal to the account address derived from `new_pubkey_hash` using
   `create2_address_zksync(creator_address, salt_arg, new_pubkey_hash, code_hash)` where all arguments are supplied by
   user and that `account_nonce == 0`.

```typescript
function pubkey_message(account_id, nonce, new_pubkey_hash, additional_data /* arbitrary 32 bytes*/): bytes {
  return concat(new_pubkey_hash, nonce, account_id, additional_data);
}
```

```typescript
function create2_address_zksync(creator_address, salt_arg /* abitrary 32 bytes */, new_pubkey_hash, code_hash): bytes {
  const salt = keccak256(salt_arg, new_pubkey_hash);
  // address is derrived using EIP-1014 https://eips.ethereum.org/EIPS/eip-1014
  return CREATE2_ADDRESS(creator_address, salt, code_hash);
}
```

#### User transaction

##### Structure

| Field                    | Value/type       | Description                                                                                   |
| ------------------------ | ---------------- | --------------------------------------------------------------------------------------------- |
| type                     | `0xf8`           | Operation code                                                                                |
| account_id               | AccountId        | Unique id of the rollup account                                                               |
| account                  | ETHAddress       | Address of the rollup account                                                                 |
| new_pubkey_hash          | RollupPubkeyHash | Hash of the new rollup public key                                                             |
| fee_token                | TokenId          | Unique token identifier in the rollup used to pay fee                                         |
| fee                      | PackedFee        | Packed amount of fee paid                                                                     |
| nonce                    | Nonce            | A one-time code that specifies the order of transactions                                      |
| valid_from               | Timestamp        | Unix timestamp from which the block with this transaction can be processed                    |
| valid_until              | Timestamp        | Unix timestamp until which the block with this transaction can be processed                   |
| signature                | Signature        | [Signature](#transaction-singature) of previous fields, see the spec below                    |
| eth_signature (optional) | ETHSignature     | Ethereum signature of the message defined above. Null if operation was authorized on contract |

##### Example

```json
{
  "type": "ChangePubKey",
  "accountId": 2061,
  "account": "0x1f04204dba8e9e8bf90f5889fe4bdc0f37265dbb",
  "newPkHash": "sync:63aa2a0efb97064e0e52a6adb63a42018bd6e72b",
  "feeToken": 1023,
  "fee": "56700000000",
  "nonce": 155724003,
  "signature": {
    "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
    "signature": "a44999d6d7962b8216ebb2c53afc159f4bd85282844b1e443f82a1e50bfe1c2b70377ac0dbe1b8c14672fac3548df77f0b7bae0eb40c553191b9b0606ef86b04"
  },
  "ethSignature": null,
  "ethAuthData": {
    "type": "Onchain"
  },
  "validFrom": 0,
  "validUntil": 18446744073709551615
}
```

Signed transaction representation.

```
Signer:
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signed transaction fields:
type: 0xf8
version: 0x01
accountId: 0x0000080d
account: 0x1f04204dba8e9e8bf90f5889fe4bdc0f37265dbb
new_pub_key_hash: 0x63aa2a0efb97064e0e52a6adb63a42018bd6e72b
token: 0x000003ff
fee: 0x46e8
nonce: 0x094828e3
time_range: 0x0000000000000000ffffffffffffffff
Signed bytes: 0xf8010000080d1f04204dba8e9e8bf90f5889fe4bdc0f37265dbb63aa2a0efb97064e0e52a6adb63a42018bd6e72b000003ff46e8094828e30000000000000000ffffffffffffffff
```

#### Rollup operation

##### Structure

| Field | Value/type     | Description               |
| ----- | -------------- | ------------------------- |
| tx    | ChangePubkeyTx | Transaction defined above |

#### Circuit constraints

```python
# ChangePkOp - Rollup operation described above
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

account = get_account_tree(ChankgePkOp.tx.account_id)
fee_account = get_tree_account(Block.fee_account)

fee = unpack_fee(WithdrawOp.tx.packed_fee)

def tree_invariants():
    account.id == ChangePkOp.tx.id
    account.address == ChangePkOp.tx.account
    account.nonce < MAX_NONCE
    account.nonce == ChangPkOp.tx.nonce
    account.balance[ChangPkOp.tx.fee_token] >= fee
    ChangePkOp.tx.new_pubkey_hash == recover_signer_pubkey_hash(ChangePkOp.tx)

def tree_updates():
    account.balance[ChangPkOp.tx.fee_token] -= fee
    account.pubkey_hash = ChangePkOp.tx.new_pubkey_hash
    account.nonce += 1

    fee_account.balance[ChangePkOp.token] += fee

def pubdata_invariants():
    OnchainOp.opcode == 0x07
    OnchainOp.account_id == ChanePkOp.tx.account_id
    OnchainOp.new_pubkey_hash == ChangePkOp.tx.new_pubkey_hash
    OnchainOp.account_address == ChangePkOp.tx.account
    OnchainOp.nonce == ChangePkOp.tx.nonce
    OnchainOp.fee_token == ChangePkOp.tx.fee_token
    OnchainOp.packed_fee == ChangePkOp.tx.packed_fee
```

#### Signature validity

Signature validity is verified twice:

1. Rollup signature is verified in circuit.
2. Ethereum signature is verified when transaction is committed to the Ethereum.

### 10. Forced exit

#### Description

Withdraws funds from unowned Rollup account to appropriate balance of the Ethereum address, corresponding to a Rollup
account. Unowned account is an account with no signing key set.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 6      | 51                |

##### Structure

| Field                | Byte len | Value/type  | Description                                                                                |
| -------------------- | -------- | ----------- | ------------------------------------------------------------------------------------------ |
| opcode               | 1        | `0x08`      | Operation code                                                                             |
| initiator_account_id | 4        | AccountId   | Unique identifier of the rollup account which initiates a forced exit operation (sender)   |
| target_account_id    | 4        | AccountId   | Unique identifier of the rollup account to perform a forced exit on (target)               |
| token                | 4        | TokenId     | Unique token identifier in the rollup                                                      |
| full_amount          | 16       | StateAmount | Full amount of funds sent                                                                  |
| packed_fee           | 2        | PackedFee   | Packed amount of fee paid                                                                  |
| target_address       | 20       | EthAddress  | The address of Ethereum account, to the balance of which funds will be accrued (recipient) |

##### Example

```
0800000004000000050002000000000000000002c68af0bb1400000012080910111213141516171819202122233425262800000000
```

Reads as: account #4 initiates a transfer for account #5 of token #2 amount 0x000000000000000002c68af0bb140000 for fee
packed in representation 0x0012 for the Rollup account which has Ethereum account address
0x0809101112131415161718192021222334252628.

#### User transaction

##### Structure

| Field                | Value/type | Description                                                                                  |
| -------------------- | ---------- | -------------------------------------------------------------------------------------------- |
| type                 | `0x08`     | Operation code                                                                               |
| initiator_account_id | AccountId  | Unique id of the sender rollup account in the state tree                                     |
| target               | ETHAddress | Unique address of the rollup account from which funds will be withdrawn (forced exit target) |
| token                | TokenId    | Unique token identifier in the rollup                                                        |
| fee                  | PackedFee  | Packed amount of fee paid                                                                    |
| nonce                | Nonce      | A one-time code that specifies the order of transactions                                     |
| signature            | Signature  | [Signature](#transaction-singature) of previous fields, see the spec below                   |

##### Example

User transaction representation. (NOTE: tx bytecode differs slightly from this representation due to data packing, see
the spec below).

```json
{
  "initiatorAccountId": 4118,
  "target": "0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513",
  "token": 100,
  "fee": "56700000000",
  "nonce": 352676723,
  "signature": {
    "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
    "signature": "a8e4e6b1d90c6a7ffb7786c8560bc22c6e2668d6c7675bff2e956b7511a9139eb3068127385e4905a2d597aa2bd9d630540d069634f5c2d76c64a6a322730d00"
  }
}
```

Signed transaction representation.

```
Signed using:
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

type: 0xf7
version: 0x01
account_id: 0x00001016
target: 0xdc8f1d4d7b5b4cde2dbc793c1d458f8916cb0513
token: 0x00000064
fee: 0x46e8
nonce: 0x15056b73
Signed bytes: 0xf70100001016dc8f1d4d7b5b4cde2dbc793c1d458f8916cb05130000006446e815056b73
```

#### Rollup operation

##### Structure

| Field | Value/type   | Description                                  |
| ----- | ------------ | -------------------------------------------- |
| tx    | ForcedExitTx | Signed forced exit transaction defined above |

#### Circuit constraints

```python
# ForcedExitOp - Rollup operation described above
# Block - block where this Rollup operation is executed
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum
# ZeroPubkey - Value determining an unset signing key of an account

initiator_account = get_tree_account(ForcedExitOp.tx.account_id)
target_account = get_account_by_address(ForcedExitOp.tx.target)
fee_account = get_tree_account(Block.fee_account)

target_account_initial_balance = target_account.balances[ForcedExitOp.tx.token]
fee = unpack_fee(ForcedExitOp.tx.packed_fee)

def tree_invariants():
    ForcedExitOp.token < MAX_FUNGIBLE_TOKENS

    initiator_account.nonce == ForcedExitOp.nonce
    initiator_account.nonce < MAX_NONCE
    initiator_account.balance[ForcedExitOp.tx.token] >= fee
    initiator_account.pubkey_hash == recover_signer_pubkey_hash(ForcedExitOp.tx)
    target_account.pubkey_hash == ZeroPubkey


def tree_updates():
    initiator_account.balance[WithdrawOp.tx.token] -= fee
    initiator_account.nonce += 1

    target_account.balance[WithdrawOp.tx.token] = 0

    fee_account.balance[WithdrawOp.token] += fee

def pubdata_invariants():
    OnchainOp.opcode == 0x08
    OnchainOp.initiator_account_id == WithdrawOp.tx.initiator_account_id
    OnchainOp.token == WithdrawOp.tx.token
    OnchainOp.full_amount == target_account_initial_balance
    OnchainOp.packed_fee == WithdrawOp.tx.packed_fee
    OnchainOp.target == WithdrawOp.tx.target
```

### 11. Swap

#### Description

Performs an atomic swap of tokens between 2 Rollup accounts at an arranged ratio.

#### Onchain operation

##### Size

| Chunks | Significant bytes |
| ------ | ----------------- |
| 5      | 46                |

##### Structure

| Field       | Byte len | Value/type     | Description                                                                              |
| ----------- | -------- | -------------- | ---------------------------------------------------------------------------------------- |
| opcode      | 1        | `0x0b`         | Operation code                                                                           |
| account_a   | 4        | AccountId      | Unique identifier of the first rollup account that performs a swap                       |
| recipient_a | 4        | AccountId      | Unique identifier of the rollup account which receives the funds sent by `account_b`     |
| account_b   | 4        | AccountId      | Unique identifier of the second rollup account that performs a swap                      |
| recipient_b | 4        | AccountId      | Unique identifier of the rollup account which receives the funds sent by `account_a`     |
| submitter   | 4        | AccountId      | Unique identifier of the rollup account which submits the swap transaction and pays fees |
| token_a     | 4        | TokenId        | Unique identifier of the token that account_a is swapping                                |
| token_b     | 4        | TokenId        | Unique identifier of the token that account_b is swapping                                |
| fee_token   | 4        | TokenId        | Unique identifier of the token in which submitter is paying fees                         |
| amount_a    | 5        | PackedTxAmount | Full amount of funds sent by `account_a` (of `token_a`)                                  |
| amount_b    | 5        | PackedTxAmount | Full amount of funds sent by `account_b` (of `token_b`)                                  |
| packed_fee  | 2        | PackedFee      | Packed amount of fee paid                                                                |
| nonce_mask  | 1        | 1 Byte         | Nonce mask <sup>\*</sup>                                                                 |

<sup>\*</sup> Nonce mask is an 8-bit number. 1st bit set indicates that account_a's nonce was incremented. 2nd bit set
indicates that account_b's nonce was incremented. Other bits are always 0.

##### Example

```
0b000000050000000600000007000000080000002a00000007000000010000002d00000012200000001b2005800200000000
```

Reads as: account #5 has swapped amount 0x0000001220 in packed representation of token #7 for account #7's amount
0x0000001b20 in packed representation of token #1. account #6 received the swapped tokens #1, and account #8 received
swapped tokens #7. account #42 has submitted the swap and payed the fee of 0x0580 in packed representation with a token
\#45. account #5's nonce has not been incremented, while account #7's has.

#### User transaction

##### Order structure

| Field       | Value/type     | Description                                                                 |
| ----------- | -------------- | --------------------------------------------------------------------------- |
| type        | `0x6f`         | Operation code                                                              |
| account_id  | AccountId      | Unique id of the sender rollup account in the state tree                    |
| recipient   | ETHAddress     | Unique address of the rollup account that will receive the funds            |
| nonce       | Nonce          | A one-time code that specifies the order of transactions                    |
| token_sell  | TokenId        | Unique identifier of the token to be swapped                                |
| token_buy   | TokenId        | Unique identifier of the token to be swapped for                            |
| amount      | PackedTxAmount | Amount of funds to be swapped, 0 indicates a limit order                    |
| ratio       | Ratio          | Array of 2 15-byte values, acceptable sell:buy ratio                        |
| valid_from  | Timestamp      | Unix timestamp from which the block with this transaction can be processed  |
| valid_until | Timestamp      | Unix timestamp until which the block with this transaction can be processed |
| signature   | Signature      | [Signature](#transaction-singature) of previous fields, see the spec below  |

##### Swap structure

| Field             | Value/type     | Description                                                                |
| ----------------- | -------------- | -------------------------------------------------------------------------- |
| type              | `0xf4`         | Operation code                                                             |
| submitter_id      | AccountId      | Unique id of the sender rollup account in the state tree                   |
| submitter_address | ETHAddress     | Unique address of the rollup account that submitted the transaction        |
| nonce             | Nonce          | A one-time code that specifies the order of transactions                   |
| orders_hash       | Hash           | Rescue hash of 2 serialized concatenated orders <sup>\*</sup>              |
| fee_token         | TokenId        | Unique identifier of the token in which submitter is paying fees           |
| fee               | PackedFee      | Packed amount of fee paid                                                  |
| amount_a          | PackedTxAmount | Amount of funds to be swapped by `account_a`                               |
| amount_b          | PackedTxAmount | Amount of funds to be swapped by `account_b`                               |
| signature         | Signature      | [Signature](#transaction-singature) of previous fields, see the spec below |

<sup>\*</sup> In the submitted transaction both orders are present in their full form, but only their hash is signed by
the user.

##### Example

User transaction representation. (NOTE: tx bytecode differs slightly from this representation due to data packing, see
the spec below).

```json
{
  "submitterId": 16777214,
  "submitterAddress": "0xa143fc5851d93cdc72e90d63cfb7286395ce20d5",
  "nonce": 2081453911,
  "orders": [
    {
      "accountId": 4,
      "recipient": "0x204dba8e9e8bf90f5889fe4bdc0f37265dbb05e3",
      "nonce": 412184582,
      "tokenBuy": 1023,
      "tokenSell": 1023,
      "ratio": ["12", "18"],
      "amount": "12000000000",
      "validFrom": 0,
      "validUntil": 18446744073709551615,
      "signature": {
        "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
        "signature": "8bfc1b854d6cf4f6d30a98249562d34c4b2c72344498a5803817ca994f6f051b984052dd501b10d2567a24719043de92bd1d7a55a951e7be9cc0995873f88002"
      }
    },
    {
      "accountId": 16777214,
      "recipient": "0x50dfcd4ee9ca4f2039d58883631f0460e0e0668f",
      "nonce": 119487968,
      "tokenBuy": 1023,
      "tokenSell": 1023,
      "ratio": ["18", "12"],
      "amount": "18000000000",
      "validFrom": 0,
      "validUntil": 18446744073709551615,
      "signature": {
        "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
        "signature": "ce385443624ae362991fd6150d950aea8143ae66388b36b4ec2ccac2c2b80a19fee8c56fcf693158f655f8612fbc3374b046b22673bd9907b31e13bdb61cc301"
      }
    }
  ],
  "amounts": ["12000000000", "18000000000"],
  "fee": "56000000",
  "feeToken": 1023,
  "signature": {
    "pubKey": "0e1390d3e86881117979db2b37e40eaf46b6f8f38d2509ff3ecfaf229c717b9d",
    "signature": "5b64500bf4fae9766792129808f8116a8d63a9c35dda9378e986ba708f983a9d5bd0d8ffb59aaa1b3f47f42ae8bc629cd19514f6a72fc82a144591c16105b605"
  }
}
```

Signed transaction representation.

```
Signer (account_a):
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signer (account_b):
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signer (submitter):
Private key: Fs(0x057afe7e950189b17eedfd749f5537a88eb3ed4981467636a115e5c3efcce0f4)
Public key: x: Fr(0x0e63e65569365f7d2db43642f9cb15781120364f5e993cd6822cbab3f86be4d3), y: Fr(0x1d7b719c22afcf3eff09258df3f8b646af0ee4372bdb7979118168e8d390130e)

Signed transaction fields:
type: 0xf4
version: 0x01
submitterId: 0x00fffffe
submitterAddress: 0xa143fc5851d93cdc72e90d63cfb7286395ce20d5
nonce: 0x7c107757
orders_hash: 0x8d2f227c69cb8d3dc16e20cb1b6829d1ee5d97d276ada6c6e6f71e3bccdc17
fee_token: 0x000003ff
fee: 0x4605
amounts[0]: 0x59682f0000
amounts[1]: 0x861c468000
Signed bytes: 0xf40100fffffea143fc5851d93cdc72e90d63cfb7286395ce20d57c1077578d2f227c69cb8d3dc16e20cb1b6829d1ee5d97d276ada6c6e6f71e3bccdc17000003ff460559682f0000861c468000
```

#### Rollup operation

##### Structure

| Field | Value/type | Description                           |
| ----- | ---------- | ------------------------------------- |
| tx    | SwapTx     | Signed swap transaction defined above |

#### Circuit constraints

```python
# SwapOp - Rollup operation described above
# Block - block where this Rollup operation is executed
# OnchainOp - public data created after executing this rollup operation and posted to the Ethereum

account_a = get_tree_account(SwapOp.orders.0.account_id)
account_b = get_tree_account(SwapOp.orders.1.account_id)
submitter = get_tree_account(SwapOp.submitter_id)
recipient_a = get_account_by_address(SwapOp.orders.0.recipient)
recipient_b = get_account_by_address(SwapOp.orders.1.recipient)
fee_account = get_tree_account(Block.fee_account)

fee = unpack_fee(SwapOp.tx.packed_fee)

def tree_invariants():
    SwapOp.orders.0.token_sell < MAX_TOKENS
    SwapOp.orders.0.token_buy < MAX_TOKENS
    SwapOp.orders.1.token_sell < MAX_TOKENS
    SwapOp.orders.1.token_buy < MAX_TOKENS
    SwapOp.fee_token < MAX_FUNGIBLE_TOKENS

    submitter.nonce == SwapOp.nonce
    account_a.nonce == SwapOp.orders.0.nonce
    account_b.nonce == SwapOp.orders.1.nonce
    submitter.nonce < MAX_NONCE
    account_a.nonce < MAX_NONCE
    account_b.nonce < MAX_NONCE

    submitter.pubkey_hash == recover_signer_pubkey_hash(SwapOp)
    account_a.pubkey_hash == recover_signer_pubkey_hash(SwapOp.orders.0)
    account_b.pubkey_hash == recover_signer_pubkey_hash(SwapOp.orders.1)

    # tokens must match
    SwapOp.orders.0.token_buy == SwapOp.orders.1.token_sell
    SwapOp.orders.1.token_sell == SwapOp.orders.0.token_buy
    SwapOp.orders.0.token_buy != SwapOp.orders.0.token_sell

    # if partial order filling is not allowed, amounts must match
    SwapOp.orders.0.amount == 0 or SwapOp.orders.0.amount == SwapOp.amounts.0
    SwapOp.orders.1.amount == 0 or SwapOp.orders.1.amount == SwapOp.amounts.1

    # either one of 2 proposed prices is accepted (or in between)
    SwapOp.amounts.0 * SwapOp.orders.0.ratio.1 <= SwapOp.amounts.1 * SwapOp.orders.0.ratio.0
    SwapOp.amounts.1 * SwapOp.orders.1.ratio.1 <= SwapOp.amounts.0 * SwapOp.orders.1.ratio.0

    # balance checks
    account_a.balance[SwapOp.orders.0.token_sell] >= SwapOp.amounts.0
    account_b.balance[SwapOp.orders.1.token_sell] >= SwapOp.amounts.1
    submitter.balance[SwapOp.fee_token] >= SwapOp.fee

    if account_a.id == submitter.id and SwapOp.fee_token == SwapOp.orders.0.token_sell:
        account_a.balance[SwapOp.orders.0.token_sell] >= SwapOp.amounts.0 + SwapOp.fee

    if account_b.id == submitter.id and SwapOp.fee_token == SwapOp.orders.1.token_sell:
        account_b.balance[SwapOp.orders.1.token_sell] >= SwapOp.amounts.1 + SwapOp.fee


def tree_updates():
    account_a.balance[SwapOp.orders.0.token_sell] -= SwapOp.amounts.0
    recipient_b.balance[SwapOp.orders.1.token_buy] += SwapOp.amounts.0
    account_b.balance[SwapOp.orders.1.token_sell] -= SwapOp.amounts.1
    recipient_a.balance[SwapOp.orders.0.token_buy] += SwapOp.amounts.1

    submitter.balance[SwapOp.fee_token] -= SwapOp.fee
    fee_account.balance[SwapOp.fee_token] += SwapOp.fee

    submitter.nonce += 1
    if SwapOp.orders.0.amount != 0 and submitter.id != account_a.id:
        account_a.nonce += 1
    if SwapOp.orders.1.amount != 0 and submitter.id != account_b.id:
        account_b.nonce += 1

def pubdata_invariants():
    OnchainOp.opcode == 0x0b
    OnchainOp.account_a == SwapOp.orders.0.account_id
    OnchainOp.recipient_a == SwapOp.orders.0.recipient
    OnchainOp.account_b == SwapOp.orders.1.account_id
    OnchainOp.recipient_b == SwapOp.orders.1.recipient
    OnchainOp.submitter == SwapOp.submitter_id
    OnchainOp.token_a == SwapOp.orders.0.token_sell
    OnchainOp.token_b == SwapOp.orders.1.token_sell
    OnchainOp.fee_token == SwapOp.fee_token
    OnchainOp.packed_amount_a == SwapOp.amounts.0
    OnchainOp.packed_amount_b == SwapOp.amounts.1
    OnchainOp.packed_fee == SwapOp.fee
    OnchainOp.nonce_mask == (SwapOp.orders.0.amount != 0) | (SwapOp.orders.1.amount != 0) << 1
```

## Smart contracts API

### Rollup contract

#### Deposit Ether

Deposit Ether to Rollup - transfer Ether from user L1 address into Rollup address

```solidity
function depositETH(address _zkSyncAddress) payable
```

- `_zkSyncAddress`: The receiver Layer 2 address

msg.value equals amount to deposit.

#### Deposit ERC-20 token

Deposit ERC-20 token to Rollup - transfer token from user L1 address into Rollup address

```solidity
function depositERC20(IERC20 _token, uint104 _amount, address _zkSyncAddress)
```

- `_token`: Token address in L1 chain
- `_amount`: Amount to deposit
- `_zkSyncAddress`: The receiver Rollup address

#### Withdraw Ether

Withdraw NFT to L1

```solidity
function withdrawPendingNFTBalance(uint32 _tokenId)
```

- `_tokenId`: Token to withdraw

#### Withdraw ERC-20 token

Withdraw ERC20 token to L1 - Transfer token from contract to owner

```solidity
function withdrawPendingBalance(
  address payable _owner,
  address _token,
  uint128 _amount
)
```

- `_owner`: Recipient
- `_token`: Token address in L1 chain
- `_amount`: Amount to withdraw

#### Authenticate rollup public key change

Authenticates pubkey hash change for new rollup public key.

```solidity
function setAuthPubkeyHash(bytes calldata _pubkey_hash, uint32 _nonce)
```

- `_pubkey_hash`: `RollupPubkeyHash`
- `_nonce`: Account nonce for which this pubkey change is authorized.

#### Censorship resistance

Register full exit request to withdraw all token balance from the account. The user needs to call it if she believes
that her transactions are censored by the validator.

```solidity
function requestFullExit(uint32 _accountId, address _token)
```

- `_accountId`: `AccountId` of the Rollup account
- `_token`: Token address in L1 chain

Register full exit request to withdraw NFT tokens balance from the account. Users need to call it if they believe that
their transactions are censored by the validator.

```solidity
function requestFullExitNFT(uint32 _accountId, uint32 _tokenId)
```

- `_accountId`: `AccountId` of the Rollup account
- `_tokenId`: NFT Token ID

#### Exodus mode

##### Withdraw funds

Withdraws token from Rollup to L1 in case of exodus mode. User must provide proof that she owns funds.

```solidity
function performExodus(
    StoredBlockInfo memory _storedBlockInfo,
    address _owner,
    uint32 _accountId,
    uint32 _tokenId,
    uint128 _amount,
    uint32 _nftCreatorAccountId,
    address _nftCreatorAddress,
    uint32 _nftSerialId,
    bytes32 _nftContentHash,
    uint256[] memory _proof
)
```

- `_storedBlockInfo`: Stored block data of the last verified block
- `_owner`: owner address in L2 chain
- `_accountId`: `AccountId` of the owner account
- `_tokenId`: Verified token id
- `_amount`: `StateAmount` Full amount of the given token that belong to `AccountId` in the last verified block
- `_nftCreatorAccountId`: Creator id of NFT (optional, only for NFT)
- `_nftCreatorAddress`: Creator Address of NFT (optional, only for NFT)
- `_nftSerialId`: Serial ID of NFT (optional, only for NFT)
- `_nftContentHash`: Identifier of the NFT represented as a 32-byte hex string
- `_proof`: Proof that user funds are present in the account tree

##### Cancel outstanding deposits

Cancels open priority requests, accrues users balances from deposit priority requests in Exodus mode.

```solidity
function cancelOutstandingDepositsForExodusMode(uint64 _n, bytes[] memory _depositsPubdata)
```

- `_n`: Supposed number of requests to cancel (if there are fewer requests than the provided number - all of the
  requests will be canceled) number of requests)
- `_depositsPubdata`: The array of the pubdata for the deposits to be cancelled. Please note, that it should not contain
  any (even empty) pubdata for `FullExit` priority operations.

#### Rollup Operations

##### Commit block

Submit committed block data. Only active validator can make it. Onchain operations will be checked on contract and
fulfilled on block verification.

```solidity
struct StoredBlockInfo {
  uint32 blockNumber;
  uint64 priorityOperations;
  bytes32 pendingOnchainOperationsHash;
  uint256 timestamp;
  bytes32 stateHash;
  bytes32 commitment;
}

struct OnchainOperationData {
  bytes ethWitness;
  uint32 publicDataOffset;
}

struct CommitBlockInfo {
  bytes32 newStateHash;
  bytes publicData;
  uint256 timestamp;
  OnchainOperationData[] onchainOperations;
  uint32 blockNumber;
  uint32 feeAccount;
}

function commitBlocks(StoredBlockInfo memory _lastCommittedBlockData, CommitBlockInfo[] memory _newBlocksData)

```

- `_lastCommittedBlockData`: Stored info of the last committed block.
- `_newBlocksData`: Data of the new committed blocks.

`StoredBlockInfo` - block data that we store on Ethereum. We store hash of this structure in storage and pass it in tx
arguments every time we need to access any of its field.

- `blockNumber` - rollup block number
- `priorityOperations` - number of priority operations committed in this block
- `pendingOnchainOperationsHash` - hash of all onchain operations that have to be processed when block is finalized
  (executed)
- `timestamp` - block timestamp
- `stateHash` - root hash of the rollup account tree state
- `commitment` - rollup block commitment.

`OnchainOperationData` - data needed for the onchain operation processing.

- `ethWitness` - additional data that can be needed for the onchain operation processing (e.g. signature for change
  pubkey operation)
- `publicDataOffset`- offset in the public data bytes after which pubdata for the onchain operation begins

`CommitBlockInfo` - data needed for new block commit

- `newStateHash` - new rollup state root hash
- `publicData` - public data of the executed rollup operations
- `timestamp` - rollup block timestamp
- `onchainOperations` - list of onchain operations that needs to be processed
- `blockNumber` - rollup block number
- `feeAccount` - id of the fee account for rollup block

##### Verify block commitments

Verify aggregated proof of multiple block commitments.

```solidity
function verifyAggregatedBlockProof(
    uint256[] memory _recursiveInput,
    uint256[] memory _proof,
    uint8[] memory _vkIndexes,
    uint256[] memory _individualVksInputs,
    uint256[16] memory _subproofsLimbs
)
```

- `_recursiveInput` - input of the recursive aggregated circuit
- `_proof` - recursive proof
- `_vkIndexes` - index of the verification keys in the aggregated verification keys tree
- `_individualVksInputs` - block commitments to be verified
- `_subproofsLimbs` - data needed for recursive proof verification

##### Execute blocks

Finishes block processing after block commitments were verified onchain. Only active validator can make it. This block
onchain operations will be fulfilled.

```solidity
struct ExecuteBlockInfo {
  StoredBlockInfo storedBlock;
  bytes[] pendingOnchainOpsPubdata;
}

function executeBlocks(ExecuteBlockInfo[] memory _blocksData)

```

- `_blocksData` - data needed to finish block execution

`ExecuteBlockInfo`

- `storedBlock` - stored block that was committed
- `pendingOnchainOpsPubdata` - list of pubdata for onchain operations that needs to be processed

#### Exodus mode trigger

Checks if Exodus mode must be entered. Exodus mode must be entered in case of current ethereum block number is higher
than the oldest of existed priority requests expiration block number.

```solidity
function activateExodusMode() returns (bool)
```

#### Revert blocks

Revert blocks that were not verified before deadline determined by `EXPECT_VERIFICATION_IN` constant. The caller must be
valid operator.

```solidity
function revertBlocks(StoredBlockInfo[] memory _blocksToRevert)
```

- `_blocksToRevert`: committed blocks to revert in reverse order starting from last committed.

### Governance contract

#### Change governor

Change current governor. The caller must be current governor.

```solidity
function changeGovernor(address _newGovernor)
```

- `_newGovernor`: Address of the new governor

#### Add token

Add token to the list of networks tokens. The caller must be current token governance.

```solidity
function addToken(address _token)
```

- `_token`: Token address

#### Pause token

Add token to the list of paused tokens. The caller must be current governor. Its impossible to create deposits of the
paused tokens.

```solidity
function setTokenPaused(address _tokenAddr, bool _tokenPaused);

```

- `_tokenAddr`: Token address
- `_tokenPaused`: Paused status of the token

#### Set validator

Change validator status (active or not active). The caller must be current governor.

```solidity
function setValidator(address _validator, bool _active)
```

- `_validator`: Validator address
- `_active`: Active flag

##### Change token governance

```solidity
function changeTokenGovernance(TokenGovernance _newTokenGovernance)
```

- `_newTokenGovernance`: New token Governance

#### Check for governor

Validate that specified address is the token governance address

```solidity
function requireGovernor(address _address)
```

- `_address`: Address to check

#### Check for active validator

Validate that specified address is the active validator

```solidity
function requireActiveValidator(address _address)
```

- `_address`: Address to check

#### Check that token id is valid

Validate token id (must be less than total tokens amount).

```solidity
function isValidTokenId(uint32 _tokenId) returns (bool)
```

- `_tokenId`: Token id

Returns: bool flag that indicates if token id is less than total tokens amount.

#### Check that token address is valid

Validate token address (it must be presented in tokens list).

```solidity
function validateTokenAddress(address _tokenAddr) returns (uint16)
```

- `_tokenAddr`: Token address

Returns: token id.

#### Register creator corresponding to the factory

Register custom factory for withdrawing NFT

```solidity
function registerNFTFactoryCreator(uint32 _creatorAccountId, address _creatorAddress, bytes memory _signature)
```

- `_creatorAccountId`: Creator's zkSync account ID
- `_creatorAddress`: NFT creator address
- `_signature`: Creator's signature

The creator should sign the message

```solidity
    "\x19Ethereum Signed Message:\n141",
    "\nCreator's account ID in zkSync: ",
    Bytes.bytesToHexASCIIBytes(abi.encodePacked((_creatorAccountId))),
    "\nCreator: ",
    Bytes.bytesToHexASCIIBytes(abi.encodePacked((_creatorAddress))),
    "\nFactory: ",
    Bytes.bytesToHexASCIIBytes(abi.encodePacked((_factoryAddress)))
```

This signature should be sent to the governance contract by calling `registerNFTFactoryCreator` from the factory
contract.

#### Set default NFT factory

Register factory, which will be use for withdrawing NFT by default

```solidity
function setDefaultNFTFactory(address _factory)
```

- `_factory`: NFT factory address

#### Get NFT factory for creator

Get NFT factory which will be used for withdrawing NFT for corresponding creator

```solidity
function getNFTFactory(uint32 _creatorAccountId, address _creatorAddress)
```

- `_creatorAccountId`: Creator account id
- `_creatorAddress`: Creator address

### Token Governance contract

#### Add token

Collects fees for adding a token and passes the call to the `addToken` function in the governance contract.

```solidity
function addToken(address _token)
```

- `_token`: ERC20 token address

#### Set listing fee token

Set new listing token and fee, can be called only by zkSync governor.

```solidity
function setListingFeeToken(IERC20 _newListingFeeToken, uint256 _newListingFee)
```

- `_newListingFeeToken`: address of the token in which fees will be collected
- `_newListingFee`: amount of tokens that will need to be paid for adding tokens

#### Set listing fee

Set new listing fee, can be called only by zkSync governor.

```solidity
function setListingFee(uint256 _newListingFee)
```

- `_newListingFee`: amount of tokens that will need to be paid for adding tokens

#### Set lister

Enable or disable token lister, if enabled new tokens can be added by that address without payment, can be called only
by zkSync governor.

```solidity
function setLister(address _listerAddress, bool _active)
```

- `_listerAddress`: address that can list tokens without fee
- `_active`: active flag

#### Set listing cap

Change maximum amount of tokens that can be listed using this method, can be called only by zkSync governor.

```solidity
function setListingCap(uint16 _newListingCap)
```

- `_newListingCap`: max number of tokens that can be listed using this contract

#### Set treasury

Change address that collects payments for listing tokens, can be called only by zkSync governor.

```solidity
function setTreasury(address _newTreasury)
```

- `_newTreasury`: address that collects listing payments

## Block state transition circuit

Block circuit describes state transition function (STF) from previous state to the new one by applying a number of
transactions.

Public inputs:

- pub_data_commitment: commitment to the state transition of the block; this is a hash that includes `old_root`,
  `new_root`, `block_number`, `validator_address`, `pub_data_rolling_hash` (see smart the contract code)., `timestamp`,
  `onhain_ops_offset_commitment`

Witness:

- old_root,
- new_root,
- block_number,
- validator_address,
- pub_data,
- pub_data_rolling_hash,
- list of transactions,
- state Merkle trees.

If the proof is valid (the circuit is satisfied), it means that there exists a set of transactions which transitions the
state from the previous one (cryptographically fingerprinted by the Merkle root `old_root`) into the new one
(cryptographically fingerprinted by the Merkle root `new_root`) such that concatenated `pub_data` of this transactions
in the order of application is cryptographically fingerprinted by `pub_data_commitment`. `oncahin_ops_offset_commitment`
is array of bytes where one byte corresponds to each public data chunk and is set to 1 if chunk is first chunk for the
onchain operation and 0 otherwise.

## Appendix I: Cryptographic primitives

### Rescue hash

For Merkle trees and hash invocations that require collision resistance we use Rescue hash described in
[AABDS19](https://eprint.iacr.org/2019/426.pdf). Reference implementation examples can be found in
[SW19](https://starkware.co/hash-challenge-implementation-reference-code/#marvellous).

For our application we've chosen the following parametrization:

- rate = 2
- capacity = 1
- non-linearity of 5th degree for BN254 curve

MDS matrix and round constants are generated from the seed phrase using the following
[code](https://github.com/matter-labs/franklin-crypto/blob/186e1241373616ac99f6f84d688905cf9bb6aa0c/src/rescue/bn256/mod.rs#L48).
Seen phrases for round constants and MDS matrix consist of two parts: short human-readable and long abstract that was
taken from ZCash's original "sapling-crypto" library and over which we could not have any influence. For MDS matrix seed
phrase is chosen to be the first one from the series that generates matrix without eigenvalues.

If number of hashed elements it not divisible by rate then extra field element equal to `1` is appended to the hash
input.

Number of input elements if internally forbidded to be equal to zero.

Outputs of Rescue hash are expected to be uniformly distributed in the field (but their bits are not uniformly
distributed).

Rate of 2 allows us to get up to two field elements from the sponge per hash round.

#### Test vectors

In test vectors we output only first element of the squeezed sponge. Such operation mode

| **Example 1** |                                                                      |
| ------------- | -------------------------------------------------------------------- |
| Msg length    | 1 field element                                                      |
| Msg           | [0x27014c0bd27dddc8514b53831287e0ba02b26875bdcb34f0d4699681f487cf7b] |
| Hash          | 0x1c54bc6adef0a488caa8ef6723ae30c784ddb0659effe5c4d0ea19b5e038300a   |

<!-- prettier-ignore-start -->
| **Example 2** | |
|-|-|
|Msg length|2 field elements|
|Msg|[0x27014c0bd27dddc8514b53831287e0ba02b26875bdcb34f0d4699681f487cf7b, 0x238ba289e8783d31585aa75bba8ddc2269c0c2d8c45d0769943b16f009ff5510]|
|Hash|0x1a751dc151d807fcb5269089c4d120ef318e26f2eaea983d74096f577cb45d93|

| **Example 3** | |
|-|-|
|Msg length|3 field elements|
|Msg|[0x27014c0bd27dddc8514b53831287e0ba02b26875bdcb34f0d4699681f487cf7b, 0x238ba289e8783d31585aa75bba8ddc2269c0c2d8c45d0769943b16f009ff5510, 0x069fd7f225dd46f03e4e0059d187419eb51b5ab5a33368e4ac05e62353dda0c3]|
|Hash|0x2c3045ae4008cab38d00491870f9cb3aecb63d56c7199fd922af7b92e00722b6|
<!-- prettier-ignore-end -->

### Bitpacking

Rescue is an algebraic hash that operates over field elements, so any binary data (packed transaction) has first to be
encode into the series of field elements. For this bit string `x` is encoded as series of field elements placing `253`
bits into each element starting from the LSB.

### Transaction signature

Signature is made according to derandomized Simple Schnorr Multi-Signature(i.e., a protocol which allows a group of
signers to produce a short, joint signature on a common message) called MuSig. You can find its complete description in
the relevant article [MPSW18](https://eprint.iacr.org/2018/068.pdf). Also if you only need an algorithm for
implementation read [DF08](https://www.politesi.polimi.it/bitstream/10589/144372/1/main.pdf) page 53. Note: you need
some background in mathematics, elliptic curve cryptography and knowledge of Schnorr signature algorithm to completely
understand how to implement MuSig. [DF08](https://www.politesi.polimi.it/bitstream/10589/144372/1/main.pdf) contains all
the necessary basics for this purpose.

Signature is formed over the BabyJubjub curve in a normalized form (`-x^2 + y^2 = 1 + dx^2y^2`, so `a = -1`):

- Base field = Scalar field of BN254 (group order)
- `d = 12181644023421730124874158521699555681764249180949974110617291017600649128846`
- main subgroup order = `2736030358979909402780800718157159386076813972158567259200215660948447373041`

#### Derandomization

Derandomization is made via [RFC6979](https://tools.ietf.org/html/rfc6979) using SHA-256 hash function according to
[RFC4634](https://tools.ietf.org/html/rfc4634) pages 4-7. Derandomization comes down to the fact that we are not
generating randomness using a hash function based on some entropy and the message. Actually we generate some number k,
using HMAC mechanism (read [RFC2104](https://tools.ietf.org/html/rfc2104) page 3) and
[SHA-256](https://tools.ietf.org/html/rfc4634) as hash function for message in algorithm of k generation (find in
[RFC6979](https://tools.ietf.org/html/rfc6979) pages 10-13) and replace randomness with this number.

#### Challenge

For Schnorr signature (that MuSig is) we need to generate Fiat-Shamir transformation challenge that should be a random
bit string of some length (120 bits or more). We label such a challenge `c`. For this we use the following procedure.
Later for `message` we assume a signed message (transaction hash).

- Define `pad32(X)` pads byte string `X` to 32 bytes with zero bytes and panics on longer strings. Inside of the circuit
  all inputs are of the fixed length.
- `EncodeLE(F)` encodes a field element `F` of as a byte string in little-endian order
- `XCoord(P)` takes `X` coordinate of the point `P` on BabyJubjub curve
- `EncodeIntoFr(X)` encodes byte string `X` as a series of field elements of the circuit base field `Fr`. Each byte is
  represented as a series of 8 bits (MSB first) and such full series of bits (from all the bytes) is packed into field
  elements using bitpacking as described above.
- For shortness `enc(X)` = `EncodeIntoFr(pad32(EncodeLE(X)))`
- we absorb the series of field elements into the Rescue sponge as
  `sponge = Rescue( enc(XCoord(PubKey)), enc(XCoord(R)), enc(message))`
- we draw two field elements `F0, F1` from the sponge (that has `rate = 2`)
- Bottom 125 bits of `F0` are placed into bottom 125 bits of `c` (LSB of `F0` is placed into LSB of `c`)
- Bottom 125 bits of `F1` are placed into next 125 bits of `c` (LSB of `F1` is placed into 126th bit of `c`)

Such procedure allows us to get `c` uniformly in `[0, 2^250)`.

#### Dealing with subgroup

BabyJubjub curve is a Twisted Edwards curve that always have a subgroup of even order. In the signature we require that
public key and signature's random point `R` belong to the main subgroup (of prime order).

#### Test vectors

- Secret key = `0x05368b800322fad50e74d9b1eab3364570d67a56ef133de0c8dbf1deaf2a474e`
- Public key X = `0x2d3801d48de21c009f4329af753cc554793c98c0c9594f4a20f7e7d23608d69d`
- Public key Y = `0x2e55266f6fb271ccd351cbb10cf953adafcafe7c6785375f583611063ddf93cb`

| **Example 1** |                                                                    |
| ------------- | ------------------------------------------------------------------ |
| Msg length    | 0 bytes                                                            |
| Msg           |                                                                    |
| Signature R.X | 0x27b3b852c85cedfdcba33a6efe4f54207f91bd84c0b5849c4ffc9682f3c9e25f |
| Signature R.Y | 0x125575f528c6df6e70f903f70b50a81bc1442a88178134c7d04b2d2f6372aec9 |
| Signature S   | 0x04ce6d6e21f874bd89293ea53050dbf742338d0b1513e450538fed7e56c8cdc9 |

| **Example 2** |                                                                    |
| ------------- | ------------------------------------------------------------------ |
| Msg length    | 1 byte                                                             |
| Msg           | 0x72                                                               |
| Signature R.X | 0x024341a6854db6897eb0fc51b9612db1d1ff5c50dfcac99bf1c2396e77f047e1 |
| Signature R.Y | 0x1ca7b1c1d7f54becbf3dbb96808139e6572aac85377a2bcda46b125db31d07b2 |
| Signature S   | 0x005694d986c7c13cb428c713b4e5f7d590d4a1561e6df64112619955c98958cb |

| **Example 3** |                                                                    |
| ------------- | ------------------------------------------------------------------ |
| Msg length    | 2 bytes                                                            |
| Msg           | 0xaf82                                                             |
| Signature R.X | 0x0dd227c7e193e87ef488783db85100a7f8ac2f020d72e154532eea3f72b58cc5 |
| Signature R.Y | 0x2abb436ba00308869e27badba36387a0981a91b4359a6bf323ae4bdf10771867 |
| Signature S   | 0x01dd611aaadce6fcea01094556585f9dac6b025831a0b07ed70edb9efcb2c7df |

### SHA256

Algorithm: SHA-256 according to [RFC4634](https://tools.ietf.org/html/rfc4634).

#### Test vectors

| **Example 1** |                                                                    |
| ------------- | ------------------------------------------------------------------ |
| Msg length    | 0 bytes                                                            |
| Msg           |                                                                    |
| Hash          | 0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855 |

| **Example 2** |                                                                    |
| ------------- | ------------------------------------------------------------------ |
| Msg length    | 1 byte                                                             |
| Msg           | 72                                                                 |
| Hash          | 0x8722616204217eddb39e7df969e0698aed8e599ba62ed2de1ce49b03ade0fede |

| **Example 3** |                                                                    |
| ------------- | ------------------------------------------------------------------ |
| Msg length    | 2 bytes                                                            |
| Msg           | 0xaf82                                                             |
| Hash          | 0x2a0305714ebec7cc0cc0949aa208aa04dc7a4b43ff0d9f4f76546ae8056e2713 |

### Sparse Merkle Tree

1. Generic SMT description
   [Ostersjo/Dahlberg, R.: Sparse Merkle Trees: Definitions and Space-Time Trade-Offs with Applications for Balloon. Bachelor’s thesis, Karlstad University (2016)](http://www.diva-portal.org/smash/get/diva2:936353/FULLTEXT02.pdf)
2. Basics of SMT
   [Fichter K.: What’s a Sparse Merkle Tree? (2018)](https://medium.com/@kelvinfichter/whats-a-sparse-merkle-tree-acda70aeb837)

In zkSync we use a sparse Merkle tree with a flexible hashing strategy. We can change its depth depending on how many
accounts we want to have.
