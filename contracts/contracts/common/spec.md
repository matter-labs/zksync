- unit conversion: different by token?
- lock funds after deposit
- authorization for ERC20 transferFrom

- max deposits/exits per block

# Events

# Todo

- add tokens+
- priority queue full exit mechanism
- update mechanism: strict opt-in only! (integrate into our circuit)
- manage validators+
- key hash
- deploy key separately?

# Governance

# Functions

- doDeposit
- doWithdraw

- depositERC20
- depositETH / default()
- withdrawERC20
- withdrawETH

- commitBlock
- verifyBlock

# Tests

- deposits
- withdraws


# Contract docs

## Deployment

## Governance
### Validators
### Tokens
### Migration (contract upgrade)

Franklin shall always have a strict opt-in policy: we guarantee that user funds are retrievable forever under the conditions a user has opted in when depositing funds, no matter what. A migration to a newer version of the contract shall be easy and cheap, but MUST require a separate opt-in or allow the user to exit.

The update mechanism shall follow this workflow:

- The network governor can schedule an update, specifying a target contract and an ETH block deadline.
- A scheduled update can not be cancelled (to proceed with migration even if exodus mode is activated while waiting for the migration; otherwise we would need to recover funds scheduled for migration with a separate procedure).
- Users can opt-in via a separate Franklin operation: move specific token balance into a subtree on a special migration account. This subtree must also maintain and update counters for total balances per token.
- The migration account MUST have a dedicated hardcoded account_id (to specify).
- When the scheduled ETH block is reached, anybody MUST be able to seal the migration.
- After the migration is sealed, anybody MUST be able to transfer total balances for each token by providing a SNARK proof of the amounts from the migration account subtree.
- When the migration is sealed, the contract enters exodus mode: whoever has not opted in can now exit. Thus, the root state will remain frozen.
- The new contract will read the latest state root directly from the old one (this is safe, because the root state is frozen and can not be changed).

## Deposits and withdrawals
### Root-chain balances
### Deposits and withdrawals workflow

## Block committment
### Franklin operations
### Root-chain holders

## Block verification

## Reverting expired blocks

## Exit queue & exodus mode
### Exit queue
### Exodus mode
