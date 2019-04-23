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

## Pub data structure

Public data is represented by a byte string with concatenated pub data of the transaction. Each pub data set is prefixed with a single byte of the type of operation.

|tx1_optype|tx1_pub_data|tx2_optype|tx2_pub_data|...

Public data of each operation is padded to the maximum pub data size of the circuit.

The size of pub data members is indicated in bytes. Pubkeys in pub data are represented by Pedersen hashes truncated to 160 bits.

## Funding circuit

Each operation in the circuit requires:
- 2 Merkle path proof checks of height 33
- 1 signature check

Public data of each operation is padded to 28 bytes.

### Circuit operations

#### deposit

Create an account and deposit a balance into it from the root chain.

|Optype         |0                                                       |
|Pub data       |account: 3, token: 1, amount: 3, pubkey_hash: 20, fee: 1|
|Pub data size  |28 bytes                                                |

Verification:
- User initiates a deposit by a transaction on the root chain which creates a deposit queue entry
- Public data for the transaction is checked by the smart contract against the deposit queue
- Signature check in circuit is ignored

#### transfer_to_new

Create a new account and transfer some balance into it from an existing one.

|Optype         |1                                                       |
|Pub data       |account: 3, token: 1, amount: 3, pubkey_hash: 20, fee: 1|
|Pub data size  |28 bytes                                                |

Verification:
- Owner of existing account signs (optype, account, token, nonce, amount, fee, pubkey_hash)

#### full_exit

Initiate full exit of all account assets to the root chain and clear the account.

|Optype         |2                                                      |
|Pub data       |account: 3, subtree_root: 20                           |
|Pub data size  |8 bytes                                                |

Verification:
- User initiates a full exit by a transaction on the root chain which creates an exit queue entry
- Public data for the transaction is checked by the smart contract against the exit queue
- Signature check in circuit is ignored

Comments:
- Account leaf and the subtree is cleared
- `account_subtree_hash` is stored in the smart contract and requires another SNARK transaction to withdraw individual balances (tbd)

#### partial_exit

Withdraw part of a particular token balance to the mainchain.

|Optype         |3                                                      |
|Pub data       |account: 3, token: 1, amount: 3, fee: 1                |
|Pub data size  |8 bytes                                                |

Verification:
- Account owner signs (optype, account, token, leaf_nonce, amount, fee)

#### escalation

Resolve state channel conflict by a smart contract on the mainnet.

|Optype         |4                                                                |
|Pub data       |account: 3, subaccount: 1, creation_nonce: 2, subaccount_nonce: 2|
|Pub data size  |26 bytes                                                         |

Verification:
- Either account owner or the co-signer signs (optype, account, subaccount, creation_nonce)

### Circuit code

```python

running_hash := initial_hash
current_root := state_merkle_root

for tx in transactions: # iterate through witness

    running_hash := accumulate(running_hash, tx.pubdata)

    # prepare variables from witness

    leaf_balance := tx.old.balance
    leaf_nonce := tx.old.leaf_nonce
    creation_nonce := tx.old.creation_nonce
    cosigner_pubkey := tx.old.cosigner_pubkey
    owner_pub_key := tx.old.owner_pub_key
    account_nonce := tx.old.account_nonce

    pubkey_hash := hash(tx.pubkey)
    cosigner_pubkey_hash := hash(cosigner_pubkey)
    check_sig(tx.sig_msg, tx.signer_pubkey) # must always be valid, but msg and signer can be dummy

    # check initial merkle paths

    full_leaf_index := tx.leaf_is_token ? tx.leaf_index : 0x100 + tx.leaf_index
    subtree_root := check_merkle_path(full_leaf_index, (leaf_balance, leaf_nonce, creation_nonce, cosigner_pubkey_hash))
    current_root := check_merkle_path(tx.account, hash(owner_pub_key, subtree_root, account_nonce))
    
    # validate operations

    deposit_correct := 
        pubdata == (tx.account, tx.leaf_index, tx.amount, pubkey_hash, tx.fee) &&
        (owner_pub_key, subtree_root, account_nonce) == EMPTY_ACCOUNT &&
        leaf_is_token
    transfer_to_new_correct := 
        deposit_correct && # same checks as for deposit operation
        sig_msg == ('transfer_to_new', tx.account, leaf_index, leaf_nonce, tx.amount, tx.fee, pubkey_hash) &&
        signer_pubkey == tx.owner_pub_key
    full_exit_correct :=
        pubdata == (tx.account, tx.subtree_root)
    partial_exit_correct := 
        pubdata == (tx.account, tx.leaf_index, tx.amount, tx.fee) &&
        leaf_is_token &&
        sig_msg == ('partial_exit', tx.account, tx.leaf_index, leaf_nonce, tx.amount, tx.fee) &&
        signer_pubkey == tx.owner_pub_key
    escalation_correct := 
        pubdata == (tx.account, leaf_index, creation_nonce, leaf_nonce) &&
        !leaf_is_token &&
        sig_msg == ('escalation', tx.account, leaf_index, creation_nonce) &&
        (signer_pubkey == tx.owner_pub_key || signer_pubkey == cosigner_pubkey)
    
    tx_correct := switch optype
        'deposit'           => deposit_correct
        'transfer_to_new'   => transfer_to_new_correct
        'full_exit'         => full_exit_correct
        'partial_exit'      => partial_exit_correct
        'escalation'        => escalation_correct
    enforce tx_correct

    # update state conditionally depending on the operation
    # `if conditon: x = y` is implemented as a binary switch: `x = condition ? y : x`

    if optype=='deposit':
        leaf_balance = leaf_balance

    #leaf_balance := optype=='transfer_to_new' ? amount : leaf_balance
    
    if optype=='full_exit':
        owner_pub_key = owner_pub_key
        account_nonce = account_nonce
        subtree_root  = EMPTY_TREE_ROOT

    # the code below allows transactions to fail to update the state if the balance is insufficient
    # this makes it possible to instantly confirm any transaction regardless of the future balance situation
    partial_exit_valid := amount <= leaf_balance
    if optype=='partial_exit' && partial_exit_valid:
        leaf_balance = leaf_balance - amount
    # nonce is always updated nonetheless
    if optype=='partial_exit':
        leaf_nonce = leaf_nonce + 1

    # ...and so on for each operation

    # check final merkle paths

    subtree_root := check_merkle_path(full_leaf_index, (leaf_balance, leaf_nonce, creation_nonce, cosigner_pubkey_hash))
    current_root := check_merkle_path(tx.account, hash(owner_pub_key, subtree_root, account_nonce))


```

## Main circuit

Each operation in the circuit requires:
- 4 Merkle path proof checks of height 33
- 4 signature checks

Public data of each operation is padded to 15 bytes + 1 byte for optype.

### Circuit code

```
current_root := state_merkle_root
for tx in transactions:
    check_merkle_path(leaf_index, leaf)
    
```

### Circuit operations

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
