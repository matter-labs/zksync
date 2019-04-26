# Franklin DEX platform

## State structure

- 2^24 accounts
- 2^9 leafs under each account: 
    - 256 for tokens (token with id 0 represents ETH)
    - 256 for subaccounts (orders/state channels)

Full Merkle tree height: 33.

<img src="https://docs.google.com/drawings/d/e/2PACX-1vQmABflC3CUHQb62x6fDmyLnVFQbqZGAoJW8j9T6WSKL-ixOtU3xLjd2_hJRCRVn2fTq17Bs1ySQUbj/pub?h=520">

([edit diagram here](https://docs.google.com/drawings/d/13bFjrSipx8-RKyAPbxzCCyXtswzvuFLjD-O8QEYaUYA/edit?usp=sharing))

Token type is determined by the index in the balance tree.

## Pub data 

### Structure

Public data is represented by a byte string with concatenated pub data of the transaction. Each pub data set is prefixed with a single byte of the type of operation.

|tx1_optype|tx1_pub_data|tx2_optype|tx2_pub_data|...

Public data of each operation is padded to the maximum pub data size of the circuit.

The size of pub data members is indicated in bytes. Pubkeys in pub data are represented by Pedersen hashes truncated to 160 bits.

### By operation

([See table here](https://docs.google.com/spreadsheets/d/1ejK1MJfVehcwjgjVDFD3E2k1EZ7auqbG_y0DKidS9nA/edit?usp=drive_open&ouid=102923468016872611309))

## Main circuit ops

Each operation in the circuit requires:
- 2 Merkle path proof checks of height 33
- 1 signature check

Public data of each operation is padded to 28 bytes.

### Circuit operations

#### padding (optype = 0)

Phony operation for block padding.

Comments:
- Optype must qual 0 so that padding can efficiently be added to the pub data before hashing in the smart contract.

#### deposit

Create an account and deposit a balance into it.

Verification:
- User initiates a deposit by a transaction on the root chain which creates a deposit queue entry
- Public data for the transaction is checked by the smart contract against the deposit queue
- Signature check in circuit is ignored

#### deposit_from

Same as deposit, but requires the operation before to be `transfer_to_new`, and balance is taken from an existing account rather than root chain.

#### transfer_to_new

Create a new account and transfer some balance into it from an existing one.

Verification:
- Owner of existing account signs (optype, account, token, nonce, amount, fee, pubkey_hash)
- Requires the subsequent operation to be `deposit`

Comments:
- Splitting into `transfer_to_new` and `deposit_from` operations is necessary because for each operation in this circuit we only update one account/balance leaf in the tree.

#### full_exit

Initiate full exit of all account assets to the root chain and clear the account.

Verification:
- User initiates a full exit by a transaction on the root chain which creates an exit queue entry
- Public data for the transaction is checked by the smart contract against the exit queue
- Signature check in circuit is ignored

Comments:
- This operation is needed to force validators to exit a single account via a priority queue
- Account leaf and the subtree is cleared
- `account_subtree_hash` is stored in the smart contract and requires another SNARK transaction to withdraw individual balances (tbd)

#### partial_exit

Withdraw part of a particular token balance to the mainchain.

Verification:
- Account owner signs (optype, account, token, leaf_nonce, amount, fee)

#### escalation

Resolve state channel conflict by a smart contract on the mainnet.

Verification:
- Either account owner or the co-signer signs (optype, account, subaccount, creation_nonce)

#### transfer

Transfer an amount of tokens from one account balance to another.

|Pub data|Total size|
|--------|----------|
|amount: 3, token: 1, from_account: 3, to_account: 3, fee: 1| 11 bytes|

Verification:
- Account owner signs (optype, from_account, to_account, token, amount, fee, nonce)

#### create_subaccount

Create subaccount to place an order or open a state channel.

|Pub data|Total size|
|--------|----------|
|account: 3, subaccount: 1, amount: 3, token: 1, cosigner_fee: 2, cosigner_account: 3, fee: 1| 14 bytes|

Verification:
- Account owner signs(optype, account, nonce, subaccount, token, amount, fee)

Comments:
- subaccounts are used for both order or state channels
- order/state channel conditions are signed and sent to the TEC offchain

#### close_subaccount

Close a subaccount to cancel an order or settle a state channel.

|Pub data|Total size|
|--------|----------|
|account: 3, subaccount: 1, cosigner_balance: 3| 7 bytes|

Verification:
- Account owner signs `signed state` at subaccount creation
- Tx sender signs(optype, account, subaccount, creation_nonce, subaccount_nonce)
- Tx sender is either the co-signer or account owner (only if the order expired)

Comments:
- cooperative closing; if co-signer doesn't cooperate, resolution via priority queue + escalation
- cosigner_balance is sent to the co-signer, the rest is sent to the account owner, subaccount leaf is cleared

#### execute_orders

Execute two orders against each other.

|Pub data|Total size|
|--------|----------|
|account1: 3, subaccount1: 1, account2: 3, subaccount2: 3, amount12: 3, amount21: 3, fee: 1| 15 bytes|

Verification:
- Account owner 1 signs `signed state` at subaccount 1 creation
- Account owner 2 signs `signed state` at subaccount 2 creation
- Cosigner 1 signs (optype, account1, subaccount1, subaccount1_nonce, transfer_amount12, transfer_amount21, fee1)
- Cosigner 2 signs the same for the opposite order

Comments:
- partial or full execution of an order against another order
- requires signatures of co-signers of both orders as TEC (trade execution coordinators)
- order amount is updated, receiving amount is accrued to the user account directly

#### update_nonce

Update a nonce of a subaccount representing a state channel.

|Pub data|Total size|
|--------|----------|
|account: 3, subaccount: 1, nonce: 2, fee: 1| 8 bytes|

Comments:
- if nonce in pub data is 0, subaccount nonce is incremented

tbd

## Full exit circuit

This circuit can be used by individual users only when the network enters a terminal recovery mode. It contains a single full exit operation for a single account.

# Todo / Questions

- subaccounts for everything (0 is default): transfers from anything to anything?
- sign(amount = 0) authorizes transfer of the entire balance?
- bitmask: explicit control of transfer permission for subaccounts (by Brecht)?
- commit-reveal against frontrunning?
- expiration for subaccounts
- replay protection after clearing an account: add global creation nonce?
