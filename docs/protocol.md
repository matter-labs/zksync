# ZK Sync Rollup Protocol

## Table of contents

  * [Table of contents](#table-of-contents)
  * [Glossary](#glossary)
  * [Design](#design)
    + [Overview](#overview)
    + [Assumptions](#assumptions)
    + [Protocol invariant claims](#protocol-invariant-claims)
  * [Data format](#data-format)
    + [Data types](#data-types)
    + [Amount packing](#amount-packing)
    + [State Merkle Tree (SMT)](#state-merkle-tree)
    + [ZK Sync block pub data format](#zk-sync-block-pub-data-format)
  * [ZK Sync operations](#zk-sync-operations)
    + [1. Noop operation](#1-noop-operation)
    + [2. Transfer](#2-transfer)
    + [3. Transfer to new](#3-transfer-to-new)
    + [4. Withdraw (Partial Exit)](#4-withdraw--partial-exit-)
    + [5. Close](#5-close)
    + [6. Deposit](#6-deposit)
    + [7. Full exit](#7-full-exit)
  * [Smart contracts API](#smart-contracts-api)
    + [Rollup contract](#rollup-contract)
      - [Deposit Ether](#deposit-ether)
      - [Deposit ERC-20 token](#deposit-erc-20-token)
      - [Withdraw Ether](#withdraw-ether)
      - [Withdraw ERC-20 token](#withdraw-erc-20-token)
      - [Censorship resistance](#censorship-resistance)
      - [Exodus mode](#exodus-mode)
      - [Rollup Operations](#rollup-operations)
    + [Priority Queue contract](#priority-queue-contract)
      - [Setup](#setup)
      - [Utility methods](#utility-methods)
      - [Exodus mode](#exodus-mode-1)
    + [Governance contract](#governance-contract)
      - [Change governor](#change-governor)
      - [Add token](#add-token)
      - [Set validator](#set-validator)
      - [Check for governor](#check-for-governor)
      - [Check for active validator](#check-for-active-validator)
      - [Check that token id is valid](#check-that-token-id-is-valid)
      - [Check that token address is valid](#check-that-token-address-is-valid)
  * [Block state transition circuit](#block-state-transition-circuit) 
  * [Appendix I: Cryptographic primitives](#appendix-i--cryptographic-primitives)
    + [Pedersen signature](#pedersen-signature)
    + [Pedersen hash](#pedersen-hash)
    + [SHA256](#sha256)
    + [EDDSA signature scheme](#eddsa-signature-scheme)
    + [Sparse Merkle Tree](#sparse-merkle-tree)

<small><i><a href='http://ecotrust-canada.github.io/markdown-toc/'>Table of contents generated with markdown-toc</a></i></small>

## Glossary

- **L1**: layer-1 blockchain (Ethereum)
- **Rollup**: layer-2 blockchain (ZK Sync)
- **Owner**: a user who controls some assets in L2.
- **Operator**: entity operating the Rollup.
- **Eventually**: happening within finite time.
- **Assets in rollup**: assets in L2 smart contract controlled by owners.
- **Rollup key**: owner's private key used to control deposited assets.
- **Pedersen signature**: the result of signing the owner's message, using his private key, used in Rollup internal transactions.

## Design

### Overview

ZK Sync implements a ZK rollup protocol (in short "rollup" below) for ETH and ERC20 fungible token transfers. 

General rollup workflow is as follows:

- Users can become owners in rollup by depositing assets from L1 or receiving a transfer from other owners.
- Owners can transfer assets to each other.
- Owners can withdraw assets under their control to an L1 address.

Rollup operation requires the assistance of an operator, who rolls transactions together, computes a zero-knowledge proof of the correct state transition, and affects the state transition by interacting with the rollup contract.

### Assumptions

Cryptography assumptions:

- DLP is unbroken.
- Pedersen hash and sha256 are collision-resistant.
- ZKP scheme used in the construction is secure (subject to a separate formal proof).

L1 blockchain assumptions:

- L1 protocol is secure.
- L1 is eventually censorship-resistant: a sufficiently highly priced L1 tx will be mined in a block within finite time.
- Owners have access to the full L1 archive (can at any time retrieve all block bodies of the L1 chain).

Operational assumptions:

- Rollup key is controlled by the owner and not compromised at all times.

### Protocol invariant claims

- [ ] 1. Continuous ownership: assets deposited in rollup are immediately under control of the specified owner.

- [ ] 2. Control: assets in rollup can not be transferred (change owner), change in value, disappear or be moved out of rollup, unless the owner initiates a corresponding action.

- [ ] 3. Eventual retrievability: assets in rollup controlled by an owner can eventually be withdrawn to the L1 address of the owner's choice, without the cooperation of any other party except L1 miners.

This includes, in particular, the following claims:

- [ ] 3.1. Double-spends in Rollup are not possible.
- [ ] 3.2. Rollup always operates under full reserve: the total supply of each asset type in rollup is equal to the sum of all its deposited amounts minus sum of all its withdrawn amounts.
- [ ] 3.3. Root hash is always correct and L2 state can not be corrupted
- [ ] 3.4. A state can always be restored from the calldata published to L1
- [ ] 3.5. The smart contract can not be locked out

## Data format

### Data types

|Type|Byte len|Encoding|Comment|
|--|--|--|--|
|AccountId|3|BE integer|Incremented number of accounts in Rollup. New account will have the next free id. Max value is 16777215|
|TokenId|2|BE integer|Incremented number of tokens in Rollup, max value is 65535|
|PackedTxAmount|5|[Parameters](#our-convertation-parameters-for-packing-amounts-and-fees)|Packed transactions amounts are represented with 40 bit (5 byte) values, encoded as mantissa * 10^exponent where mantissa is represented with 35 bits, exponent is represented with 5 bits. This gives a range from 0 to 34359738368 * 10^31, providing 10 full decimal digit precision.|
|PackedFee|2|[Parameters](#our-convertation-parameters-for-packing-amounts-and-fees)|Packed fees must be represented with 2 bytes: 5 bit for exponent, 11 bit for mantissa.|
|StateAmount|16|BE integer|State amount is represented as uint128 with a range from 0 to ~3.4 * 10^38. It allows to represent up to 3.4 * 10^20 "units" if standard Ethereum's 18 decimal symbols are used. This should be a sufficient range.|
|StateFee|16|BE integer|State fee is represented as uint128 with a range from 0 to ~3.4 * 10^38. It allows to represent up to 3.4 * 10^20 "units" if standard Ethereums 18 decimal symbols are used. This thould be a sufficient range.|
|Nonce|4|BE integer|Nonce reflects the current state of the account in the tree starting from zero. In order to apply the update of this state, it is necessary to indicate the current account nonce in the corresponding transaction, after which it will be automatically incremented. If you specify the wrong nonce, the changes will not occur.|
|RollupPubkeyHash|20|LE integer|To make a public key hash from a Rollup [public key](#generating-rollup-key-pair) apply [Pedersen hash function](#pedersen-hash) to the key and then take the last 20 bytes of the result.|
|EthAddress|20|LE integer|To make an Ethereum address from the Etherum's public key, all we need to do is to apply Keccak-256 hash function to the key and then take the last 20 bytes of the result.|
|PackedRollupPubkey|32|LE integer|A Rollup public key is the first 32 bytes of a Rollup [public key](#generating-rollup-key-pair)|
|TxHash|32|LE integer|To get hash for transaction apply [SHA256 function](#sha256) to concatenated bytes of [transaction fields](#zk-sync-operations)|
|Signature|64|LE integer|Read [Pedersen signature](#pedersen-signature)|
|BlockNumber|4|BE integer|Incremented number of Rollup blocks, max number is 4294967295|
|RootHash|32|LE integer|[Merkle tree root hash](#state-sparse-merkle-tree-smt)|

### Amount packing

Amounts and fees are compressed in ZK Sync using simple [fundamentals of floating point arithmetic](https://en.wikipedia.org/wiki/Floating-point_arithmetic).

A floating-point number has the following parts: a mantissa, a radix, and an exponent. The mantissa (always non-negative in our case) holds the significant digits of the floating-point number. The exponent indicates the power of the radix that the mantissa and sign should be multiplied by. The components are combined as follows to get the floating-point value:

```
sign * mantissa * (radix ^ exponent)
```

Mantissa and exponent parameters used in ZK Sync:

|Type|Exponent bit width|Mantissa bit width|Radix|
|--|--|--|--|
|PackedTxAmount|5|35|10|
|PackedFee|5|11|10|

### State Merkle Tree

Accounts and Balances trees representation:

![](https://i.imgur.com/itXl2UV.png)

Legend:
- Ha is account tree height.
- Hb is balance tree height.

We have directly one main `Accounts tree` and its leaves `Accounts` also have subtrees `Balances tree` with their own `Balances` leaves.

#### Leaf hash

**The leaf hash** is the [pedersen hash](#pedersen-hash) of its fields in **LE integer representation**, that are concatenated in the order presented below. 

#### Account leaf

Each account is inserted into the Accounts tree as a leaf with the least free id (`AccountId`).

|Field|Type|
|--|--|
|nonce|Nonce|
|pubkey_hash|RollupAddress|
|address|EthAddress|
|state_tree_root|RootHash|

`state_tree_root` is combined using Pedersen hash of the padded to 256 bit `balance_tree_root` and 256 zero bits reserved for future subtree root hash.

An empty leaf contains: `state_tree_root` computed using empty balances subtree, all other fields equal to zero.

#### Balance leaf

|Field|Type|
|--|--|
|value|StateAmount|

And empty leaf contains `value` equal to zero.

### ZK Sync block pub data format

Rollup block pub data consists of [Rollup operations](#zk-sync-operations) pub data sequence. The maximum block size is a constant value. If the size of the operations included in the block is not enough to fill it completely, the remainder will be filled with empty [Noop](#1-noop-operation) operations.

## ZK Sync operations

ZK Sync operations are divided into Rollup transactions (initiated inside Rollup by a Rollup account) and Priority operations (initiated on the mainchain by an Ethereum account).

Rollup transactions:

- Noop
- Transfer
- Transfer to new
- Withdraw (Partial exit)
- Close account
- Change pubkey

Priority operations:

- Deposit
- Full exit

Full list: https://docs.google.com/spreadsheets/d/1ejK1MJfVehcwjgjVDFD3E2k1EZ7auqbG_y0DKidS9nA/edit#gid=0

Legend:

- User transaction: what users can submit to the operator (body of Http request / input for contract method).
- Onchain operation: what the operator can put into the rollup block pubdata (operation pubdata).
- Node implementation: node model that describes an operation.
- Circuit implementation: circuit model that describes the operation and its witness.
- Chunk: the dimension of the operation. Each chunk has its own part of the public data (8 bytes) given through witnesses.
- Significant bytes: how many bytes, of all bytes occupied by the operation, are significant (including operation number).
- Hash: the result of SHA-256 function with operation's pubdata as input. Used for operation identification.

### 1. Noop operation

#### Description

No effects.

#### Onchain operation

##### Size

|Chunks|Significant bytes|
|--|--|
|1|1|

##### Structure

|Field|Byte len|Value/type|Description|
|--|--|--|--|
|Opcode|1|`0x00`|Operation code|

##### Example

```
0000000000000000
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

|Chunks|Significant bytes|
|--|--|
|2|16|

##### Structure

|Field|Byte len|Value/type|Description|
|--|--|--|--|
|opcode|1|`0x05`|Operation code|
|from_account|3|AccountId|Unique identifier of the rollup account from which funds will be withdrawn (sender)|
|token|2|TokenId|Unique token identifier in the rollup|
|to_account|3|AccountId|Unique identifier of the rollup account that will receive the funds (recipient)|
|packed_amount|5|PackedTxAmount|Packed amount of funds sent|
|packed_fee|2|PackedFee|Packed amount of fee paid|

##### Example

```
0500000400020000030000001ad30012
```

Reads as: transfer from account #4 token #2 to account #3 amount in packed representation 0x0000001ad3 for fee in packed representation 0x0012.

#### User transaction

##### Structure

|Field|Value/type|Description|
|--|--|--|
|type|`0x05`|Operation code|
|from_address|ETHAddress|Unique address of the rollup account from which funds will be withdrawn (sender)|
|to_address|ETHAddress|Unique address of the rollup account that will receive the funds (recipient)|
|token|TokenId|Unique token identifier in the rollup|
|amount|StateAmount|Full amount of funds sent|
|fee|StateFee|Full amount of fee paid|
|nonce|Nonce|A one-time code that specifies the order of transactions|
|signature|Signanture|Pedersen signature of previous fields that had been concatenated into a single bytes array. Before concatenation `amount` and `fee` fields are packed|

##### Example

```json
type: 5,
from: "0x03e69588c1f4155dec60da3bf5113e029911ce33",
to: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef",
token: 2,
amount: "0x0000001ad3",
fee: "0x0012",
nonce: 5,
signature: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef11036945fcc11c349c3a300f19cd87cb03c4f2ef03e69588c1f4155dec60da3bf5113e029911ce330124"
```

#### Invariants

1. Transfer.token < TotalTokens
2. from_id = get_id(Transfer.from_address) != nil
3. to_id = get_id(Transfer.to_address) != nil
4. verify(signature) == true
5. Transfer.nonce == Account(from_id).nonce
6. Account(from_id).balance(token) >= Transfer.amount + Transfer.fee

#### Tree updates

1. Account(from_id).balance(token) -= (Transfer.amount + Transfer.fee)
3. Account(from_id).nonce += 1
2. Account(to_id).balance(token) += Transfer.amount
4. Account(fees_account_id).balance(token) += Transfer.fee

### 3. Transfer to new

#### Description

Transfers funds from Rollup account to a new Rollup account (dynamically assigns a free account number to a Rollup address).
So, the "account creation" will be performed first, that is, the correspondence RollupAddress - AccountId is assigned. And then the usual funds' Transfer between Rollup accounts will occur.

#### Onchain operation

##### Size

|Chunks|Significant bytes|
|--|--|
|5|36|

##### Structure

|Field|Byte len|Value/type|Description|
|--|--|--|--|
|opcode|1|`0x02`|Operation code|
|from_account|3|AccountId|Unique identifier of the rollup account from which funds will be withdrawn (sender)|
|token|2|TokenId|Unique token identifier in the rollup|
|packed_amount|5|PackedTxAmount|Packed amount of funds sent|
|to_address|20|ETHAddress|The address that will represent the rollup account that will receive the funds (recipient)|
|to_account|3|AccountId|Unique identifier of the rollup account that will receive the funds (recipient)|
|packed_fee|2|PackedFee|Packed amount of fee paid|

##### Example

```
0200000400020000001ad30809101112131415161718192021222334252628000003001200000000
```

Reads as: transfer from account #4 token #2 amount in packed representation 0x0000001ad3 to account with address 0x0809101112131415161718192021222334252628 and id #3 for fee in packed representation 0x0012.

#### User transaction

##### Structure

|Field|Value/type|Description|
|--|--|--|
|type|`0x02`|Operation code|
|from_address|ETHAddress|Unique address of the rollup account from which funds will be withdrawn (sender)|
|to_address|ETHAddress|Unique address of the rollup account that will receive the funds (recipient)|
|token|TokenId|Unique token identifier in the rollup|
|amount|StateAmount|Full amount of funds sent|
|fee|StateFee|Full amount of fee paid|
|nonce|Nonce|A one-time code that specifies the order of transactions|
|signature|Signanture|Pedersen signature of previous fields that had been concatenated into a single bytes array. Before concatenation `amount` and `fee` fields are packed|

##### Example

Transfer to new request is the same as regular Transfer request:

```json
type: 2,
from: "0x03e69588c1f4155dec60da3bf5113e029911ce33",
to: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef",
token: 2,
amount: "0x0000001ad3",
fee: "0x0012",
nonce: 5,
signature: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef11036945fcc11c349c3a300f19cd87cb03c4f2ef03e69588c1f4155dec60da3bf5113e029911ce330124"
```

#### Invariants

1. TransferToNew.token < TotalTokens
2. from_id = get_id(TransferToNew.from_address) != nil
3. to_id = get_id(TransferToNew.to_address) == nil
4. verify(signature) == true
5. Transfer.nonce == Account(from_id).nonce
6. Account(from_id).balance(token) >= Transfer.amount + Transfer.fee

#### Tree updates

to_id = get_lowest_free_account_id()

1. Account(to_id).address = TransferToNew.to_address
2. Account(from_id).balance(token) -= (TransferToNew.amount + Transfer.fee)
3. Account(from_id).nonce += 1
4. Account(to_id).balance(token) += TransferToNew.amount
5. Account(fees_account_id).balance(token) += TransferToNew.fee

### 4. Withdraw (Partial Exit)

#### Description

Withdraws funds from Rollup account to appropriate balance of the indicated Ethereum address.

#### Onchain operation

##### Size

|Chunks|Significant bytes|
|--|--|
|6|44|

##### Structure

|Field|Byte len|Value/type|Description|
|--|--|--|--|
|opcode|1|`0x03`|Operation code|
|from_account|3|AccountId|Unique identifier of the rollup account from which funds will be withdrawn (sender)|
|token|2|TokenId|Unique token identifier in the rollup|
|full_amount|16|StateAmount|Full amount of funds sent|
|packed_fee|2|PackedFee|Packed amount of fee paid|
|to_address|20|EthAddress|The address of Ethereum account, to the balance of which funds will be accrued(recipient)|

##### Example

```
030000040002000000000000000002c68af0bb1400000012080910111213141516171819202122233425262800000000
```

Reads as: transfer from account #4 token #2 amount 0x000000000000000002c68af0bb140000 for fee packed in representation 0x0012 to ethereum account with address 0x0809101112131415161718192021222334252628.

#### User transaction

##### Structure

|Field|Value/type|Description|
|--|--|--|
|type|`0x03`|Operation code|
|from_address|ETHAddress|Unique address of the rollup account from which funds will be withdrawn (sender)|
|to_address|EthAddress|The address of Ethereum account, to the balance of which the funds will be accrued(recipient)|
|token|TokenId|Unique token identifier in the rollup|
|amount|StateAmount|Full amount of funds sent|
|fee|StateFee|Full amount of fee paid|
|nonce|Nonce|A one-time code that specifies the order of transactions|
|signature|Signanture|Pedersen signature of previous fields that had been concatenated into a single bytes array. Before concatenation `fee` field is packed|

##### Example

```json
type: 3,
from: "0x03e69588c1f4155dec60da3bf5113e029911ce33",
to: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef",
token: 2,
amount: "0x000000000000000002c68af0bb140000",
fee: "0x0012",
nonce: 5,
signature: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef11036945fcc11c349c3a300f19cd87cb03c4f2ef03e69588c1f4155dec60da3bf5113e029911ce330124"
```

#### Invariants

1. Withdraw.token < TotalTokens
2. id = get_id(Withdraw.from_address) != nil
3. verify(signature) == true
4. Transfer.nonce == Account(id).nonce
5. Account(id).balance(token) >= (Withdraw.amount + Withdraw.fee)

#### Tree updates

1. Account(id).balance(token) -= (Withdraw.amount + Withdraw.fee)
2. Account(id).nonce += 1
3. Account(fees_account_id).balance(token) += Withdraw.fee

### 6. Deposit

#### Description

Deposits funds from ethereum account to the specified Rollup account.
Deposit starts as priority operation - user calls contract method `depositEth` to deposit ethereum, or `depositErc` to deposit ERC-20 tokens. After that operator includes this operation in a block. In the account tree, the new account will be created if needed.

#### Onchain operation

##### Size

|Chunks|Significant bytes|
|--|--|
|6|42|

##### Structure

|Field|Byte len|Value/type|Description|
|--|--|--|--|
|opcode|1|`0x01`|Operation code|
|to_account|3|AccountId|Unique identifier of the rollup account that will receive the funds (recipient)|
|token|2|TokenId|Unique token identifier in the rollup|
|full_amount|16|StateAmount|Full amount of funds sent|
|to_address|20|ETHAddress|The address that will represent the rollup account that will receive the funds (recipient)|

##### Example

```
010000040002000000000000000002c68af0bb1400000809101112131415161718192021222334252628000000000000
```

Reads as: deposit to account #4 token #2 amount 0x000000000000000002c68af0bb140000, account will have address 0x0809101112131415161718192021222334252628.

#### User ethereum transaction

##### Ethereum transction

The following must be concatenated into single bytes string and placed into **transaction data field**:

|Field|Value/type|Description|
|--|--|--|
|type|`0x01`|Operation code|
|from_address|EthAddress|Ethereum account address from which funds will be withdrawn (sender) and sent to smart contract|
|token|TokenId|Unique token identifier in the rollup|
|full_amount|StateAmount|Full amount of funds sent|
|to_address|RollupAddress|The address that will represent the rollup account that will receive the funds (recipient)|

If transaction currency is Ether, provide the proper Ether amount in transaction value field.
value is full_amount

##### Example

```json
type: 1,
from_address: "0x03e69588c1f4155dec60da3bf5113e029911ce33",
token: 2,
full_amount: "0x000000000000000002c68af0bb140000",
to_address: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef",
nonce: 5,
signature: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef11036945fcc11c349c3a300f19cd87cb03c4f2ef03e69588c1f4155dec60da3bf5113e029911ce330124"
```

#### Invariants

1. FullExit.token < TotalTokens
2. id =  get_id(Deposit.account) != nil OR get_lowest_free_account_id()

#### Tree updates

1. Account(id).pubkey_hash = Deposit.to_address
2. Account(id).balance(token) += Deposit.amount

#### Censorship by the operator

It is possible that the operator for some reason does not include this operation in the block. Then, through the number of ethereum blocks set on the smart contract, the exodus mode will be launched. It will automatically return the deposit funds to the account from which they were transferred.

### 7. Full exit

#### Description

The user can request this operation to withdraw funds if he thinks that his transactions are censored by validators.

It starts as a priority operation - user calls contract method `fullExit`. After that operator includes this operation in a block.

#### Onchain operation

##### Size

|Chunks|Significant bytes|
|--|--|
|6|42|

##### Structure

|Field|Byte len|Value/type|Description|
|--|--|--|--|
|opcode|1|`0x06`|Operation code|
|account_id|3|AccountId|Unique identifier of the rollup account from which funds will be withdrawn (sender)|
|owner|20|EthAddress|The address of the fund owner account. Also to the balance of this address the funds will be accrued(recipient)|
|token|2|TokenId|Unique token identifier in the rollup|
|full_amount|16|StateAmount|Full amount of funds that had been withdrawn|

##### Example

```
060000040809101112131415161718192021222334252628000200000000000002c68af0bb1400000000
```

Reads as: full exit from account #4 with with address 0x0809101112131415161718192021222334252628, token #2, amount is 0x000000000000000002c68af0bb140000.

#### User ethereum transaction

##### Ethereum transction

The following must be concatenated into single bytes string and placed into **transaction data field**:

|Field|Value/type|Description|
|--|--|--|
|type|`0x06`|Operation code|
|account_id|AccountId|Unique identifier of the rollup account from which funds will be withdrawn (sender)|
|owner|20|EthAddress|The address of the fund owner account. Also to the balance of this address the funds will be accrued(recipient)|
|token|TokenId|Unique token identifier in the rollup|

User provides `account_id` and token address (zero address for ETH), token id is determined using governance contract and 
owner is determined using transaction sender.

##### Example

```json
type: 6,
account_id: 4
account_address: "0x11036945fcc11c349c3a300f19cd87cb03c4f2ef",
token: 2,
```

#### Invariants

1. FullExit.token < TotalTokens
2. id = get_id(FullExit.from_account) != nil
3. amount_to_withdraw = Account(id).balance(token) > 0

#### Tree updates

1. Account(id).balance(token) -= amount_to_withdraw

#### Failure signal

If something went wrong on the server side - full exit operation may be included in a block with 0 (zero) amount in pubdata.

#### Censorship by the operator

It is possible that the operator for some reason does not include this operation in the block. Then, through the number of ethereum blocks set on the smart contract, the exodus mode will be launched. After that, a user can submit exit proof to get her funds.
Read more about censorship resistance and exodus mode in special sections.

### 8. Change pubkey

#### Description

Change pubkey - changes public key of the account that is used to authorize transactions. Change pubkey is authorized
with ethereum keys for which address is the same as account address.

#### Onchain operation

##### Size

|Chunks|Significant bytes|
|--|--|
|6|48|

##### Structure

|Field|Byte len|Value/type|Description|
|--|--|--|--|
|opcode|1|`0x07`|Operation code|
|account_id|3|AccountId|Unique identifier of the rollup account|
|new_pubkey_hash|20|RollupPubkeyHash|Hash of the new rollup public key|
|account_address|20|ETHAddress|Address of the account|
|nonce|4|Nonce|Account nonce|

##### Example

```
0700000411036945fcc11c349c3a300f19cd87cb03c4f2ef03e69588c1f4155dec60da3bf5113e029911ce3300000003
```

Reads as: change pubkey, account #4, new pubkey hash sync:11036945fcc11c349c3a300f19cd87cb03c4f2ef, address: 03e69588c1f4155dec60da3bf5113e029911ce33, nonce: 3.

#### Authorization

1. Transaction can be authorized by providing signature of the message `concat[nonce, new_pubkey_hash]` (e.g. `0000000311036945fcc11c349c3a300f19cd87cb03c4f2ef` for example above) with transaction.
Transaction will be verified on the contract.
2. For users that can't sign messages it is possible to authorize this operation by calling `authPubkeyHash` method of the smart contract. User should provide new pubkey hash and nonce for this transaction.
After this transaction succeeded transaction without signature can be sent to operator.

#### User transaction

##### Structure

|Field|Value/type|Description|
|--|--|--|
|type|`0x07`|Operation code|
|account|ETHAddress|Address of the rollup account|
|new_pubkey_hash|20|RollupPubkeyHash|Hash of the new rollup public key|
|nonce|Nonce|A one-time code that specifies the order of transactions|
|signature (optional)|ETHSignanture|Ethereum signature of the message: concat[`nonce`, `new_pubkey_hash`] using keys. Null if operation was authrorized on contract. |

##### Example

```json
type: 7,
account: "0x03e69588c1f4155dec60da3bf5113e029911ce33",
newPkHash: "sync:11036945fcc11c349c3a300f19cd87cb03c4f2ef",
nonce: 5,
signature: "0x8b7385c7bb8913b9fd176247efab0ccc72e3197abe8e2d4c6596ba58a32a91675f66e80560a5f1a42bd50d58da055630ac6c18875e5ba14a362e87e903f083941c"
```

#### Invariants

1. id = get_id(ChangePubkeyHash.account) != nil
2. address == ChangePubkeyHash.account
4. Transfer.nonce == Account(id).nonce

#### Tree updates

1. Account(id).pubkey_hash = ChangePubkeyHash.new_pubkey_hash
2. Account(id).nonce += 1


## Smart contracts API

### Rollup contract

#### Deposit Ether
Deposit Ether to Rollup - transfer Ether from user L1 address into Rollup address
```solidity
depositETH(address _franklinAddr)
```
- _franklinAddr: The receiver Layer 2 address

msg.value equals amount to deposit.

#### Deposit ERC-20 token
Deposit ERC-20 token to Rollup - transfer token from user L1 address into Rollup address
```solidity
depositERC20(address _token, uint128 _amount, bytes calldata _rollupAddr) payable
```
- _token: Token address in L1 chain
- _amount: Amount to deposit 
- _rollupAddr: The receiver Rollup address

#### Withdraw Ether

Withdraw ETH to L1 - register withdrawal and transfer ether from contract to msg.sender
```solidity
withdrawETH(uint128 _amount)
```
- _amount: Amount to withdraw

#### Withdraw ERC-20 token

Withdraw ERC20 token to L1 - register withdrawal and transfer token from contract to msg.sender
```solidity
withdrawERC20(address _token, uint128 _amount)
```
- _token: Token address in L1 chain
- _amount: Amount to withdraw

#### Authenticate rollup public key change

Authenticates pubkey hash change for new rollup public key.
```solidity
function authPubkeyHash(bytes calldata _fact, uint32 _nonce) external {
```
- _fact: Rollup public key hash
- _nonce: Account nonce for which this pubkey change is authorized.

#### Censorship resistance

Register full exit request to withdraw all token balance from the account. The user needs to call it if she believes that her transactions are censored by the validator.
```solidity
fullExit (
    uint24 _accountId,
    address _token,
) payable
```
- _accountId: Numerical id of the Rollup account
- _token: Token address in L1 chain

#### Exodus mode

##### Withdraw funds

Withdraws token from Rollup to L1 in case of exodus mode. User must provide proof that she owns funds.
```solidity
exit(
    uint16 _tokenId,
    uint128 _amount,
    uint256[8] calldata _proof
)
```
- _proof: Proof that user funds are present in the account tree
- _tokenId: Verified token id
- _amount: Token amount

##### Cancel outstanding deposits

Cancels open priority requests, accrues users balances from deposit priority requests in Exodus mode.
```solidity
cancelOutstandingDepositsForExodusMode(uint64 _number)
```
- _number: Supposed number of requests to cancel (if there are less request than provided number - will be canceled exact number of requests)

#### Rollup Operations

##### Commit block

Submit committed block data. Only active validator can make it. Onchain operations will be stored on contract and fulfilled on block verification.
```solidity
commitBlock(
    uint32 _blockNumber,
    uint24 _feeAccount,
    bytes32 _newRoot,
    bytes calldata _publicData,
    bytes calldata _ethWitness,
    uint64[] calldata _ethWitnessSizes
)
```

- _blockNumber: Block number
- _feeAccount: Account to collect fees
- _newRoot: New tree root
- _publicData: Operations pubdata
- _ethWitness - data that can be used by smart contract for block commit that is posted outside of `_publicData` (e.g ETH signatures for pubkey change verification).
- _ethWitnessSizes - number of bytes from _ethWitness that is used for each onchain operation which needed them.

##### Verify block

Submit verified block proof. Only active validator can make it. This block onchain operations will be fulfilled.
```solidity
verifyBlock(uint32 _blockNumber, uint256[8] calldata _proof, bytes calldata _withdrawalsData)
```

- _blockNumber: Block number
- _proof Block proof
- _withdrawalsData Withdrawals data

### Priority Queue contract

#### Setup

Sets rollup address if it has not been set before.
```solidity
setRollupAddress(address _rollupAddress)
```

-_rollupAddress: Address of the Rollup contract

#### Utility methods

##### Is priority operation valid

Compares Rollup operation with corresponding priority requests' operation.
```solidity
isPriorityOpValid(uint8 _opType, bytes calldata _pubData, uint64 _id) returns (bool)
```

- _opType: Operation type
- _pubData: Operation pub data
- _id: Request id
  
Returns: bool flag that indicates if priority operation is valid (exists in priority requests list on the specified place)

##### Validate number of requests

Checks if provided number is less than uncommitted requests count.
```solidity
validateNumberOfRequests(uint64 _number)
```

- _number: Number of requests

#### Exodus mode

Checks if Exodus mode must be entered.
Exodus mode must be entered in case of current ethereum block number is higher than the oldest of existed priority requests expiration block number.
```solidity
triggerExodusIfNeeded() returns (bool)
```

Returns: bool flag that indicates if exodus mode must be entered.

### Governance contract

#### Change governor

Change current governor. The caller must be current governor.
```solidity
changeGovernor(address _newGovernor)
```

- _newGovernor: Address of the new governor

#### Add token

Add token to the list of networks tokens. The caller must be current governor.
```solidity
addToken(address _token)
```

- _token: Token address

#### Set validator

Change validator status (active or not active). The caller must be current governor.
```solidity
setValidator(address _validator, bool _active)
```

- _validator: Validator address
- _active: Active flag

#### Check for governor

Validate that specified address is the governor address
```solidity
requireGovernor(address _address)
```

- _address: Address to check

#### Check for active validator

Validate that specified address is the active validator
```solidity
requireActiveValidator(address _address)
```

- _address: Address to check

#### Check that token id is valid

Validate token id (must be less than total tokens amount).
```solidity
isValidTokenId(uint16 _tokenId) returns (bool)
```

- _tokenId: Token id

Returns: bool flag that indicates if token id is less than total tokens amount.

#### Check that token address is valid

Validate token address (it must be presented in tokens list).
```solidity
validateTokenAddress(address _tokenAddr) returns (uint16)
```

- _tokenAddr: Token address

Returns: token id.

## Block state transition circuit

Block circuit describes state transition function (STF) from previous state to the new one by applying a number of transactions.

Public inputs:

- pub_data_commitment: commitment to the state transition of the block; this is a hash that includes `old_root`, `new_root`, `block_number`, `validator_address`, `pub_data_rolling_hash` (see smart the contract code).

Witness:

- old_root,
- new_root,
- block_number,
- validator_address,
- pub_data,
- pub_data_rolling_hash,
- list of transactions,
- state Merkle treees.

If the proof is valid (the circuit is satisfied), it means that there exists a set of transactions which transitions the state from the previous one (cryptographically fingerprinted by the Merkle root `old_root`) into the new one (cryptographically fingerprinted by the Merkle root `new_root`) such that concatenated `pub_data` of this transactions in the order of application is cryptographically fingerprinted by `pub_data_commitment`.

## Appendix I: Cryptographic primitives

### Rescue hash

For Merkle trees and hash invocations that require collision resistance we use Rescue hash described in [AABDS19](https://eprint.iacr.org/2019/426.pdf). Reference implementation examples can be found in [SW19](https://starkware.co/hash-challenge-implementation-reference-code/#marvellous). 

For our application we've chosen the following parametrization:
- rate = 2
- capacity = 1
- non-linearity of 5th degree for BN254 curve

MDS matrix and round constants are generated from the seed phrase using the following [code](https://github.com/matter-labs/franklin-crypto/blob/186e1241373616ac99f6f84d688905cf9bb6aa0c/src/rescue/bn256/mod.rs#L48). Seen phrases for round constants and MDS matrix consist of two parts: short human-readable and long abstract that was taken from ZCash's original "sapling-crypto" library and over which we could not have any influence. For MDS matrix seed phrase is chosen to be the first one from the series that generates matrix without eigenvalues.

If number of hashed elements it not divisible by rate then extra field element equal to `1` is appended to the hash input.

Number of input elements if internally forbidded to be equal to zero.

Outputs of Rescue hash are expected to be uniformly distributed in the field (but their bits are not uniformly distributed).

Rate of 2 allows us to get up to two field elements from the sponge per hash round.

#### Test vectors

In test vectors we output only first element of the squeezed sponge. Such operation mode

|**Example 1**| |
|-|-|
|Msg length|1 field element|
|Msg|[0x27014c0bd27dddc8514b53831287e0ba02b26875bdcb34f0d4699681f487cf7b]|
|Hash|0x1c54bc6adef0a488caa8ef6723ae30c784ddb0659effe5c4d0ea19b5e038300a|

|**Example 2**| |
|-|-|
|Msg length|2 field elements|
|Msg|[0x27014c0bd27dddc8514b53831287e0ba02b26875bdcb34f0d4699681f487cf7b, 0x238ba289e8783d31585aa75bba8ddc2269c0c2d8c45d0769943b16f009ff5510]|
|Hash|0x1a751dc151d807fcb5269089c4d120ef318e26f2eaea983d74096f577cb45d93|

|**Example 3**| |
|-|-|
|Msg length|3 field elements|
|Msg|[0x27014c0bd27dddc8514b53831287e0ba02b26875bdcb34f0d4699681f487cf7b, 0x238ba289e8783d31585aa75bba8ddc2269c0c2d8c45d0769943b16f009ff5510, 0x069fd7f225dd46f03e4e0059d187419eb51b5ab5a33368e4ac05e62353dda0c3]|
|Hash|0x2c3045ae4008cab38d00491870f9cb3aecb63d56c7199fd922af7b92e00722b6|

### Bitpacking

Rescue is an algebraic hash that operates over field elements, so any binary data (packed transaction) has first to be encode into the series of field elements. For this bit string `x` is encoded as seried of field elements placing `253` bits into each element starting from the LSB.

### Transaction signature

Signature is made according to derandomized Simple Schnorr Multi-Signature(i.e., a protocol which allows a group of signers to produce a short, joint signature on a common message) called MuSig. You can find its complete description in the relevant article [MPSW18](https://eprint.iacr.org/2018/068.pdf). Also if you only need an algorithm for implementation read [DF08](https://www.politesi.polimi.it/bitstream/10589/144372/1/main.pdf) page 53. Note: you need some background in mathematics, elliptic curve cryptography and knowledge of Schnorr signature algorithm to completely understand how to implement MuSig. [DF08](https://www.politesi.polimi.it/bitstream/10589/144372/1/main.pdf) contains all the necessary basics for this purpose.

Signature is formed over the BabyJubjub curve in a normalized form (`-x^2 + y^2 = 1 + dx^2y^2`, so `a = -1`):

- Base field = Scalar field of BN254 (group order)
- `d = 12181644023421730124874158521699555681764249180949974110617291017600649128846`
- main subgroup order = `2736030358979909402780800718157159386076813972158567259200215660948447373041`
  

#### Derandomization

Derandomization is made via [RFC6979](https://tools.ietf.org/html/rfc6979) using SHA-256 hash function according to [RFC4634](https://tools.ietf.org/html/rfc4634) pages 4-7. Derandomization comes down to the fact that we are not generating randomness using a hash function based on some entropy and the message. Actually we generate some number k, using HMAC mechanism (read [RFC2104](https://tools.ietf.org/html/rfc2104) page 3) and [SHA-256](https://tools.ietf.org/html/rfc4634) as hash function for message in algorithm of k generation (find in [RFC6979](https://tools.ietf.org/html/rfc6979) pages 10-13) and replace randomness with this number.

#### Challenge

For Schnorr signature (that MuSig is) we need to generate Fiat-Shamir transformation challenge that should be a random bit string of some length (120 bits or more). We label such a challenge `c`. For this we use the following procedure. Later for `message` we assume a signed message (transaction hash).

- Define `pad32(X)` pads byte string `X` to 32 bytes with zero bytes and panics on longer strings. Inside of the circuit all inputs are of the fixed length.
- `EncodeLE(F)` encodes a field element `F` of as a byte string in little-endian order
- `XCoord(P)` takes `X` coordinate of the point `P` on BabyJubjub curve
- `EncodeIntoFr(X)` encodes byte string `X` as a series of field elements of the circuit base field `Fr`. Each byte is represented as a series of 8 bits (MSB first) and such full series of bits (from all the bytes) is packed into field elements using bitpacking as described above.
- For shortness `enc(X)` = `EncodeIntoFr(pad32(EncodeLE(X)))`
- we absorb the series of field elements into the Rescue sponge as `sponge = Rescue( enc(XCoord(PubKey)), enc(XCoord(R)), enc(message))`
- we draw two field elements `F0, F1` from the sponge (that has `rate = 2`)
- Bottom 125 bits of `F0` are placed into bottom 125 bits of `c` (LSB of `F0` is placed into LSB of `c`)
- Bottom 125 bits of `F1` are placed into next 125 bits of `c` (LSB of `F1` is placed into 126th bit of `c`)

Such procedure allows us to get `c` uniformly in `[0, 2^250)`.

#### Dealing with subgroup

BabyJubjub curve is a Twisted Edwards curve that always have a subgroup of even order. In the signature we require that public key and signature's random point `R` belong to the main subgroup (of prime order).

#### Test vectors

- Secret key = `0x05368b800322fad50e74d9b1eab3364570d67a56ef133de0c8dbf1deaf2a474e`
- Public key X = `0x2d3801d48de21c009f4329af753cc554793c98c0c9594f4a20f7e7d23608d69d`
- Public key Y = `0x2e55266f6fb271ccd351cbb10cf953adafcafe7c6785375f583611063ddf93cb`

|**Example 1**| |
|-|-|
|Msg length|0 bytes|
|Msg||
|Signature R.X|0x27b3b852c85cedfdcba33a6efe4f54207f91bd84c0b5849c4ffc9682f3c9e25f|
|Signature R.Y|0x125575f528c6df6e70f903f70b50a81bc1442a88178134c7d04b2d2f6372aec9|
|Signature S|0x04ce6d6e21f874bd89293ea53050dbf742338d0b1513e450538fed7e56c8cdc9|

|**Example 2**| |
|-|-|
|Msg length|1 byte|
|Msg|0x72|
|Signature R.X|0x024341a6854db6897eb0fc51b9612db1d1ff5c50dfcac99bf1c2396e77f047e1|
|Signature R.Y|0x1ca7b1c1d7f54becbf3dbb96808139e6572aac85377a2bcda46b125db31d07b2|
|Signature S|0x005694d986c7c13cb428c713b4e5f7d590d4a1561e6df64112619955c98958cb|

|**Example 3**| |
|-|-|
|Msg length|2 bytes|
|Msg|0xaf82|
|Signature R.X|0x0dd227c7e193e87ef488783db85100a7f8ac2f020d72e154532eea3f72b58cc5|
|Signature R.Y|0x2abb436ba00308869e27badba36387a0981a91b4359a6bf323ae4bdf10771867|
|Signature S|0x01dd611aaadce6fcea01094556585f9dac6b025831a0b07ed70edb9efcb2c7df|

### SHA256

Algorithm: SHA-256 according to [RFC4634](https://tools.ietf.org/html/rfc4634).

#### Test vectors

|**Example 1**| |
|-|-|
|Msg length|0 bytes|
|Msg||
|Hash|0xe3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855|

|**Example 2**| |
|-|-|
|Msg length|1 byte|
|Msg|72|
|Hash|0x8722616204217eddb39e7df969e0698aed8e599ba62ed2de1ce49b03ade0fede|

|**Example 3**| |
|-|-|
|Msg length|2 bytes|
|Msg|0xaf82|
|Hash|0x2a0305714ebec7cc0cc0949aa208aa04dc7a4b43ff0d9f4f76546ae8056e2713|

### Sparse Merkle Tree

1. Generic SMT description [Ostersjo/Dahlberg, R.: Sparse Merkle Trees: Definitions and Space-Time Trade-Offs with Applications for Balloon. Bachelor’s thesis, Karlstad University (2016)](http://www.diva-portal.org/smash/get/diva2:936353/FULLTEXT02.pdf)
2. Basics of SMT [Fichter K.: What’s a Sparse Merkle Tree? (2018)](https://medium.com/@kelvinfichter/whats-a-sparse-merkle-tree-acda70aeb837)

In ZK Sync we use a sparse Merkle tree with a flexible hashing strategy. We can change its depth depending on how many accounts we want to have.

