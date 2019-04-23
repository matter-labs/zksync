# Franklin DEX platform

## State structure

- 2^24 accounts
- 2^9 leafs under each account: 
    - 255 for tokens
    - 256 for subaccounts (orders/state channels)

<img src="https://docs.google.com/drawings/d/e/2PACX-1vQmABflC3CUHQb62x6fDmyLnVFQbqZGAoJW8j9T6WSKL-ixOtU3xLjd2_hJRCRVn2fTq17Bs1ySQUbj/pub?h=520">

([edit diagram here](https://docs.google.com/drawings/d/13bFjrSipx8-RKyAPbxzCCyXtswzvuFLjD-O8QEYaUYA/edit?usp=sharing))

Token type is determined by the index in the balance tree.

# Operations

## Pub data structure

Public data is represented by a single byte string with concatenated pub data of the transaction. Each pub data set is prefixed with a single byte of the type of operation.

The size of pub data members is indicated in bytes. Pubkeys in pub data are represented by Pedersen hashes truncated to 160 bits.

## Fund management circuit

Each operation in the circuit requires:
- 1x2 Merkle path proofs to the leaf.
- 1 signature check.

Public data of each operation is padded to 28 bytes + 1 byte for optype.

**Operations:**

- **0. deposit from root chain**
    - user initiates a deposit by a transaction on the root chain
    - pub data: (account: 3, token: 1, amount: 3, new_pubkey: 20, fee: 1): 28 bytes
    - signature check ignored

- **1. full exit**
    - pub data: (from_account: 3, subtree_hash: 20): 23 bytes
    - check sig(optype, from_account, nonce)

- **2. partial exit**
    - pub data: (account: 3, token: 1, amount: 3, fee: 1): 8 bytes
    - check sig(optype, account, token, nonce, amount, fee)

- **3. transfer to new account**
    - pub data: (account: 3, token: 1, amount: 3, fee: 1): 8 bytes
    - check sig(optype, account, token, nonce, amount, fee, new_pubkey)
    - `carry_new_pubkey` is set to `new_pubkey`
    - next operation must be "deposit from Franklin" (see below)

- **4. deposit from existing account**
    - pub data: (account: 3, new_pubkey: 20): 23 bytes    - requires the previous transaction setting the `carry_new_pubkey` variable
    - signature check ignored (because checked in the partial exit op)

- **5. escalation**
    - resolve state channel conflict by a smart contract on the mainnet
    - pub data: (account: 3, subaccount: 1, subaccount_nonce: 2, subtree_hash: 20): 26 bytes
    - check sig(optype, account, subaccount, creation_nonce) against either account pubkey or co-signer pubkey

## Operations circuit

Each operation in the circuit requires:
- 2x2 Merkle path proofs to the leaf.
- 4 signature checks.

Public data of each operation is padded to 15 bytes + 1 byte for optype.

**Operations:**

- **0. transfer**
    - operator will either deposit into an existing account or create a new one
    - pub data: (amount: 3, token: 1, from_account: 3, to_account: 3, fee: 1): 11 bytes
    - check sig1(optype, from_account, to_account, token, amount, fee, nonce)
    - ignore other signatures

- **1. create subaccount**
    - subaccounts are used for both order or state channels
    - order/state channel conditions are signed and sent to the TEC offchain
    - pub data: (account: 3, subaccount: 1, amount: 3, token: 1, cosigner_fee: 2, cosigner_account: 3, fee: 1): 14 bytes
    - check sig1(optype, account, nonce, subaccount, token_type, amount, fee) against account pub key
    - ignore other signatures

- **2. close subaccount**
    - cooperative closing; if co-signer doesn't cooperate, resolution via priority queue + escalation
    - pub data: (account: 3, subaccount: 1, cosigner_balance: 3): 7 bytes
    - check sig1(optype, account, subaccount, creation_nonce, subaccount_nonce) against either co-signer pubkey, or, after expiration, against account pub key
    - check sig2(signed state 1) against account1 pubkey
    - ignore other signatures
    - cosigner_balance is sent to the co-signer, the rest is sent to the account owner, subaccount leaf is cleared

- **3. execute order**
    - partial or full execution of an order against another order
    - requires signatures of co-signers of both orders as TEC (trade execution coordinators)
    - order amount is updated, receiving amount is accrued to the user account directly
    - pub data: (account1: 3, subaccount1: 1, account2: 3, subaccount2: 3, amount12: 3, amount21: 3, fee: 1): 15 bytes
    - check sig1(signed state 1) against account1 pubkey
    - check sig2(signed state 2) against account2 pubkey
    - check sig3(optype, account1, subaccount1, subaccount1_nonce, transfer_amount12, transfer_amount21, fee1) against cosigner1_pubkey
    - check sig4(same for account 2) against cosigner2_pubkey

- **4. subaccount transfer**
    - pub data: (account: 3, subaccount: 1, to_account: 3, amount: 3, fee: 1)
    - tbd

- **5. update nonce**
    - pub data: (account: 3, subaccount: 1, nonce: 2, fee: 1) -- if nonce in pub data is 0, subaccount nonce is incremented
    - tbd

# Todo / Questions

- subaccounts for everything (0 is default)
- sign(amount = 0) authorizes full transfer
- bitmask: allow to transfer (Brecht)
