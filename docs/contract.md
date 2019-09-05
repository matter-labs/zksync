# Franllin ETH contract documentation

## Deployment

The contract must be deployed specifying the initial ("genesis") state root hash, appointing the **network governor** (see the "Governance" section), and linking the exit queue (see the "Cenosorship resistance" section).

## Governance

Governance of the network will be excerised from a separate contract registered in the Franklin contract as `networkGovernor`. It has the power to:

- Change the set of validators.
- Add new tokens (tokens can not be removed after being added).
- Initiate migration to a new contract (see the "Migration" section).

## Deposits

To make deposit, a user can:
- Either send ETH to smart contract (will be handled by the default function),
- or call `depositERC20()` function to perform transferFrom for a registered ERC20 token. Note: the user must have previously called approve() on the ERC20 token contract in order to authorize Franklin contract to perform this operation.

This deposit creates a **root-chain balance** for the user. To move funds into Franklin, a user must submit a separate signed **circuit operation** `deposit` to the validators. In order to give the validator chance to safely include deposits in the next block, upon each root-chain deposit the **root-chain balance** is locked for a short number of ETH blocks (`LOCK_DEPOSITS_FOR` constant).

When a validator commits a block which contains a **circuit operation** `deposit`, the requested amount is moved from the **root-chain balance** to a separately stored **onchain operation**. If the block is verified, the **onchain operations** are simply discarded. If the block is reverted, the funds held by the **onchain operations** are returned to the owners' **root-chain balances**.

Note: although any validator can try to include locked deposited funds from any user into their block without permission, this situation does not require a special treatment. It constitutes a general DOS attack on the network (because the validator won't be able to prove the authorization), and thus will be ruled out in the generic fashion.

## Withdrawals

The withdrawals workflow is similar to deposits, but with reverse order. When a block with a `withdraw` **circuit operation** is committed, an **onchain operation** for the withdrawal is created. If the block is verified, funds from the **onchain operation** are acrued to the users' *root-chain balance*. If the block is reverted, the **onchain operation** is simply discarded.

A user can withdraw funds from the **root-chain balance** at any time by calling a `withdrawETH()` or `withdrawERC20()` function, unless the balance is locked by a preceding onchain deposit.

## Block committment

Only a sender from the validator set can commit a block.

The number of committed but unverified blocks is limited by `MAX_UNVERIFIED_BLOCKS` constant in order to make sure that gas is always sufficient to revert all committed blocks if the verification has not happened in time.

## Block verification

Anybody can perform verification for the committed block.

## Reverting expired blocks

If the first committed block was not verified within `EXPECT_VERIFICATION_IN` ETH blocks, all unverified blocks are moved to a separate list `blocksToRevert`. After that, anybody can release the funds held in **onchain operations** by each block by calling a `revertBlock()` function (the validator who created the block is supposed to do this, because they will get compensation for the money spent on the block -- to be implemented for multi-validator version).

## Cenosorship resistance

To enforece censorship-resistance and enable guaranteed retrievability of the funds, Franklin employs the mechanisms of **Exit queue** (soft enforcement) and **Exodus mode** (hard enforcement).

### Exit queue

If a user is being ignored by all validators, in order to get back her funds she can always submit a request into the **Exit queue**. This queue will be implemented in a separate contract.

On each committment, the Franklin contract MUST check with the **Exit queue** if there are **circuit operations** required to be included in the block. A block committment without requried operations MUST be rejected.

If the exit queue is being processed too slow, it will trigger the **Exodus mode** in the Franklin contract.

### Exodus mode

In the **Exodus mode**, the contract freezes all block processing, and all users must exit. All existing block committments can be reverted after they expire (thus no special handling is required).

Every user will be able to submit a SNARK proof that she owns funds in the latest verified state of Franklin by calling `exit()` function. If the proof verification succeeds, the entire user amount will be accrued to her **onchain balance**, and a flag will be set in order to prevent double exits of the same token balance.

## Migration

(to be implemented later)

Franklin shall always have a strict opt-in policy: we guarantee that user funds are retrievable forever under the conditions a user has opted in when depositing funds, no matter what. A migration to a newer version of the contract shall be easy and cheap, but MUST require a separate opt-in or allow the user to exit.

The update mechanism shall follow this workflow:

- The **network governor** can schedule an update, specifying a target contract and an ETH block deadline.
- A scheduled update can not be cancelled (to proceed with migration even if exodus mode is activated while waiting for the migration; otherwise we would need to recover funds scheduled for migration with a separate procedure).
- Users can opt-in via a separate Franklin operation: move specific token balance into a subtree on a special migration account. This subtree must also maintain and update counters for total balances per token.
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
