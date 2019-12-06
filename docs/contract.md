# ZKSync ETH contract documentation

![ZKSync Contract Onchain Operations](https://i.imgur.com/Y3taY1y.png)

## Deployment

The contract must be deployed specifying the initial ("genesis") state root hash, appointing the **network governor** (see the "Governance" section), and linking the exit queue (see the "Cenosorship resistance" section).

## Governance

Governance of the network will be excerised from a separate contract registered in the ZKSync contract as `networkGovernor`. It has the power to:

- Change the set of validators.
- Add new tokens (tokens can not be removed after being added).
- Initiate migration to a new contract (see the "Migration" section).

## Cenosorship resistance

To enforece censorship-resistance and enable guaranteed retrievability of the funds, ZKSync employs the mechanisms of **Priority queue** (soft enforcement) and **Exodus mode** (hard enforcement).

## Deposits

To make deposit, a user can:
- Either send ETH to smart contract (will be handled by the default function),
- or call `depositERC20()` function to perform transferFrom for a registered ERC20 token. Note: the user must have previously called approve() on the ERC20 token contract in order to authorize ZKSync contract to perform this operation.

This deposit creates **deposit priority request** that is placed in corresponding priority requests mapping and also emits **NewPriorityRequest(opType, pubData, expirationBlock)** event to notify validators that they must include this request to upcoming blocks. Complete **PriorityQueue** logic that handles **priority requests** is described in **Priority Requests** section.

When a validator commits a block which contains **circuit operations** `deposit`, **deposit onchain operation** for this deposit is created to verify compliance with priority queue requests. If it succeeds than their count will be added to **priority requests** count for this block. If the block is verified, **deposit onchain operations** and **deposit priority request** are simply discarded. 

If the block is reverted **deposit onchain operations** are siply discarded.

If ZKSync contract has entered Exodus mode and the block is unverified, the funds held by this blocks' **Deposit priority requests** are accrued to the owners' **root-chain balances** to make them possible to withdraw. This **withdraw onchain operations** and **full exit priority requests** are simply discarded.

## Withdrawals

### Partial withdrawal

It is a standard withdrawal operation. When a block with `partial_exit` **circuit operation** is committed, **withdraw onchain operation** for this withdrawal is created. If the block is verified, funds from the **withdrawal onchain operation** are acrued to the users' **root-chain balances**. 

If the block is reverted, this **withdraw onchain operations** are simply discarded.

A user can withdraw funds from the **root-chain balance** at any time by calling a `withdrawETH()` or `withdrawERC20()` function.

### Full exit

User can request this expensive operation to withdraw funds if he thinks that his transactions are censored by validators.

The user must send a transaction to **ZKSync** contract function `registerFullExit()`. This function creates **full exit priority request** that is placed in corresponding priority requests mapping and also emits **NewPriorityRequest(serialId, opType, pubData, expirationBlock)** event to notify validators that they must include this request to upcoming blocks. Complete **PriorityQueue** logic that handles **priority requests** is described in **Priority Requests** section.

When a validator commits a block which contains a **circuit operation** `full_exit`, the corresponding **withdraw onchain operation** for this withdrawal is created to verify compliance with priority queue requests. If it succeeds than their count will be added to **priority requests** count for this block. If the block is verified, funds from the **withdrawal onchain operation** are accrued to the users' **root-chain balances** and **withdraw onchain operations** and **full exit priority requests** are simply discarded.

If the block is reverted, this **withdraw onchain operations** are simply discarded.

If ZKSync contract has entered Exodus mode and the block is unverified, this **withdraw onchain operations** and **full exit priority requests** are simply discarded.

## Block committment

Only a sender from the validator set can commit a block.

The number of committed but unverified blocks is limited by `MAX_UNVERIFIED_BLOCKS` constant in order to make sure that gas is always sufficient to revert all committed blocks if the verification has not happened in time.

## Block verification

Anybody can perform verification for the committed block.

## Reverting expired blocks

If the first committed block was not verified within `EXPECT_VERIFICATION_IN` ETH blocks, all unverified blocks will be reverted and the funds held by **onchain operations** and **priority requests** will be released and stored on **root-chain balances**..

## Priority queue

This queue will be implemented in separate contract to ensure that priority operations like `deposit` and `full_exit` will be processed in a timely manner and will be included in one of ZKSync's blocks (a situation that leads to the need to reverse blocks will not happen), and also, in the case of `full_exit` transactions, the user can always withdraw funds (censorship-resistance). Its' functionality is divided into 2 parts: **Requests Queue** and **Exodus Mode**.

**NewPriorityRequest** event is emitted when a user send according transaction to ZKSync contract. Also some info about it will be stored in the mapping (operation type and expiration block) strictly in the order of arrival.
**NewPriorityRequest** event structure:
- `serialId` - serial id of this priority request
- `opType` - operation type
- `pubData` - request data
- `expirationBlock` - the number of Ethereum block when request becomes expired
`expirationBlock` is calculated as follows:
`expirationBlock = block.number + 250` - about 1 hour for the transaction to expire, `block.number` - current Ethereum block number.

When corresponding transactions are found in the commited block, their count must be recorded. If the block is verified, this count of the satisfied **priority requests** is removed from mapping. 

If the block is reverted via Exodus Mode, the funds held by **Deposit priority requests** from this block are accrued to the owners' **root-chain balances** to make them possible to withdraw. And this **Deposit priority requests** will be removed from mapping. 

### Fees for Priority Requests

In order to send priority request, the _user_ MUST pay some extra fee. That fee will be subtracted from the amount of Ether that the user sent to Deposit or Full Exit funcitons. That fee will be the payment for the _validator’s_ work to include these transactions in the block. One transaction fee is calculated as follows:
`fee = FEE_COEFF * (BASE_GAS + gasleft) * gasprice`, where
- `FEE_COEFF` - fee coefficient for priority request transaction
- `BASE_GAS` - base gas cost for transaction (usually 21000)
- `gasleft` - remaining gas for transaction code execution
- `gasprice` - gas price of the transaction

If the user sends more Ether than necessary, the difference will be returned to him.

### **Validators'** responsibility

**Validators** MUST subscribe for `NewPriorityRequest` events in the RIGHT order to include priority transactions in some upcoming blocks.
The need for _Validators_ to include these transactions in blocks as soon as possible is dedicated by the increasing probability of entering the **Exodus Mode** (described below).

### Exodus mode

If the **Requests Queue** is being processed too slow, it will trigger the **Exodus mode** in **ZKSync** contract. This moment is determined by the first (oldest) **priority request** with oldest `expirationBlock` value . If `current ethereum block number >= oldest expiration block number` the **Exodus Mode** will be entered.

In the **Exodus mode**, the contract freezes all block processing, and all users must exit. All existing block commitments will be reverted.

Every user will be able to submit a SNARK proof that she owns funds in the latest verified state of ZKSync by calling `exit()` function. If the proof verification succeeds, the entire user amount will be accrued to her **onchain balance**, and a flag will be set in order to prevent double exits of the same token balance.

## Migration

(to be implemented later)

ZKSync shall always have a strict opt-in policy: we guarantee that user funds are retrievable forever under the conditions a user has opted in when depositing funds, no matter what. A migration to a newer version of the contract shall be easy and cheap, but MUST require a separate opt-in or allow the user to exit.

The update mechanism shall follow this workflow:

- The **network governor** can schedule an update, specifying a target contract and an ETH block deadline.
- A scheduled update can not be cancelled (to proceed with migration even if exodus mode is activated while waiting for the migration; otherwise we would need to recover funds scheduled for migration with a separate procedure).
- Users can opt-in via a separate ZKSync operation: move specific token balance into a subtree on a special migration account. This subtree must also maintain and update counters for total balances per token.
- The migration account MUST have a dedicated hardcoded account_id (to specify).
- When the scheduled ETH block is reached, anybody MUST be able to seal the migration.
- After the migration is sealed, anybody MUST be able to transfer total balances for each token by providing a SNARK proof of the amounts from the migration account subtree.
- When the migration is sealed, the contract enters exodus mode: whoever has not opted in can now exit. Thus, the root state will remain frozen.
- The new contract will read the latest state root directly from the old one (this is safe, because the root state is frozen and can not be changed).

## Todo / Questions / Unsorted

- introduce key hash?
- deploy keys separately?
- unit conversion: different by token?
- describe authorization for ERC20 transferFrom
- enforce max deposits/exits per block in the circuit
