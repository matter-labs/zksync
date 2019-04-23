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

Create an account and deposit a balance into it.

|Pub data |account: 3, token: 1, amount: 3, pubkey_hash: 20, fee: 1| 28 bytes|

Verification:
- User initiates a deposit by a transaction on the root chain which creates a deposit queue entry
- Public data for the transaction is checked by the smart contract against the deposit queue
- Signature check in circuit is ignored

#### deposit_from

Same as deposit, but requires the operation before to be `transfer_to_new`, and balance is taken from an existing account rather than root chain.

#### transfer_to_new

Create a new account and transfer some balance into it from an existing one.

|Pub data |account: 3, token: 1| 4 bytes|

Verification:
- Owner of existing account signs (optype, account, token, nonce, amount, fee, pubkey_hash)
- Requires the subsequent operation to be `deposit`

Comments:
- Splitting into `transfer_to_new` and `deposit_from` operations is necessary because for each operation in this circuit we only update one account/balance leaf in the tree.

#### full_exit

Initiate full exit of all account assets to the root chain and clear the account.

||Pub data |account: 3, subtree_root: 20| 8 bytes||

Verification:
- User initiates a full exit by a transaction on the root chain which creates an exit queue entry
- Public data for the transaction is checked by the smart contract against the exit queue
- Signature check in circuit is ignored

Comments:
- Account leaf and the subtree is cleared
- `account_subtree_hash` is stored in the smart contract and requires another SNARK transaction to withdraw individual balances (tbd)

#### partial_exit

Withdraw part of a particular token balance to the mainchain.

|Pub data |account: 3, token: 1, amount: 3, fee: 1| 8 bytes|

Verification:
- Account owner signs (optype, account, token, leaf_nonce, amount, fee)

#### escalation

Resolve state channel conflict by a smart contract on the mainnet.

|Pub data |account: 3, subaccount: 1, creation_nonce: 2, subaccount_nonce: 2| 26 bytes|

Verification:
- Either account owner or the co-signer signs (optype, account, subaccount, creation_nonce)

#### padding

Phony operation for block padding.

|Pub data | | 0 bytes|

### Circuit code

```python

running_hash := initial_hash
current_root := state_merkle_root
carry := 0

for tx in transactions: # iterate through witness

    # running hash: ignoring padding transactions

    running_hash := optype == 'padding' ? running_hash : accumulate(running_hash, tx.pubdata)

    # initialize variables from witness

    leaf_balance    := tx.balance
    leaf_nonce      := tx.leaf_nonce
    creation_nonce  := tx.creation_nonce
    cosigner_pubkey := tx.cosigner_pubkey
    owner_pub_key   := tx.owner_pub_key
    account_nonce   := tx.account_nonce

    amount  := tx.amount
    fee     := tx.fee
    pubkey  := tx.pubkey

    # hashes

    pubkey_hash := hash(tx.pubkey)
    cosigner_pubkey_hash := hash(cosigner_pubkey)

    # range checks

    subtractable := amount <= leaf_balance

    # check carry from previous transaction

    carry_valid := carry == false || optype=='deposit_from' # carry only allowed to be set for deposits
    enforce carry_valid

    (amount, fee, pubkey_hash) = carry
    carry = 0

    # check signature

    check_sig(tx.sig_msg, tx.signer_pubkey) # must always be valid, but msg and signer can be phony

    # check initial merkle paths

    full_leaf_index := tx.leaf_is_token ? tx.leaf_index : 0x100 + tx.leaf_index
    subtree_root := check_merkle_path(
        full_leaf_index, 
        (leaf_balance, leaf_nonce, creation_nonce, cosigner_pubkey_hash, cosigner_balance, token))
    current_root := check_merkle_path(tx.account, hash(owner_pub_key, subtree_root, account_nonce))
    
    # validate operations

    deposit_valid := 
        (optype == 'deposit' || optype == 'deposit_from') &&
        pubdata == (tx.account, tx.leaf_index, tx.amount, pubkey_hash, tx.fee) &&
        (owner_pub_key, subtree_root, account_nonce) == EMPTY_ACCOUNT &&
        leaf_is_token

    transfer_to_new_valid := 
        optype == 'transfer_to' &&
        pubdata == (tx.account, tx.leaf_index) &&
        subtractable &&
        leaf_is_token &&
        deposit_valid && # same checks as for deposit operation
        sig_msg == ('transfer_to_new', tx.account, leaf_index, account_nonce, tx.amount, tx.fee, pubkey_hash) &&
        signer_pubkey == tx.owner_pub_key

    full_exit_valid :=
        optype == 'full_exit' &&
        pubdata == (tx.account, tx.subtree_root)

    partial_exit_valid := 
        optype == 'partial_exit' &&
        pubdata == (tx.account, tx.leaf_index, tx.amount, tx.fee) &&
        subtractable &&
        leaf_is_token &&
        sig_msg == ('partial_exit', tx.account, tx.leaf_index, account_nonce, tx.amount, tx.fee) &&
        signer_pubkey == tx.owner_pub_key

    escalation_valid := 
        optype == 'escalation' &&
        pubdata == (tx.account, leaf_index, creation_nonce, leaf_nonce) &&
        !leaf_is_token &&
        sig_msg == ('escalation', tx.account, leaf_index, creation_nonce) &&
        (signer_pubkey == tx.owner_pub_key || signer_pubkey == cosigner_pubkey)
    
    padding_valid := 
        optype == 'padding'

    tx_valid := 
        deposit_valid ||
        transfer_to_new_valid ||
        full_exit_valid ||
        partial_exit_valid ||
        escalation_valid ||
        padding_valid
    
    enforce tx_valid

    # update state conditionally depending on the operation

    # NOTE: `if conditon: x = y` is implemented as a binary switch: `x = condition ? y : x`

    if deposit_valid:
        leaf_balance = leaf_balance

    if transfer_to_new_valid:
        leaf_balance = leaf_balance - amount
        account_nonce = account_nonce + 1
        carry = (amount, fee, pubkey_hash)

    if full_exit_valid:
        owner_pub_key = 0
        account_nonce = 0
        subtree_root  = EMPTY_TREE_ROOT

    if partial_exit_valid:
        leaf_balance = leaf_balance - amount
        account_nonce = leaf_nonce + 1

    if escalation_valid:
        leaf_balance = 0
        leaf_nonce = 0
        creation_nonce = 0
        cosigner_pubkey_hash = EMPTY_HASH

    # check final merkle paths

    subtree_root := check_merkle_path(
        full_leaf_index, 
        (leaf_balance, leaf_nonce, creation_nonce, cosigner_pubkey_hash, cosigner_balance, token))
    current_root := check_merkle_path(tx.account, hash(owner_pub_key, subtree_root, account_nonce))

# at the end of the loop carry must be cleared
enforce carry == 0
enforce current_root == new_state_root
enforce running_hash == pubdata_hash

```

## Main circuit

Each operation in the circuit requires:
- 4 Merkle path proof checks of height 33
- 4 signature checks

Public data of each operation is padded to 16 bytes.

### Circuit operations

#### transfer

Transfer an amount of tokens from one account balance to another.

|Pub data |amount: 3, token: 1, from_account: 3, to_account: 3, fee: 1| 11 bytes|

Verification:
- Account owner signs (optype, from_account, to_account, token, amount, fee, nonce)

#### create_subaccount

Create subaccount to place an order or open a state channel.

|Pub data |account: 3, subaccount: 1, amount: 3, token: 1, cosigner_fee: 2, cosigner_account: 3, fee: 1| 14 bytes|

Verification:
- Account owner signs(optype, account, nonce, subaccount, token, amount, fee)

Comments:
- subaccounts are used for both order or state channels
- order/state channel conditions are signed and sent to the TEC offchain

#### close_subaccount

Close a subaccount to cancel an order or settle a state channel.

|Pub data |account: 3, subaccount: 1, cosigner_balance: 3| 7 bytes|

Verification:
- Account owner signs `signed state` at subaccount creation
- Tx sender signs(optype, account, subaccount, creation_nonce, subaccount_nonce)
- Tx sender is either the co-signer or account owner (only if the order expired)

Comments:
- cooperative closing; if co-signer doesn't cooperate, resolution via priority queue + escalation
- cosigner_balance is sent to the co-signer, the rest is sent to the account owner, subaccount leaf is cleared

#### execute_orders

Execute two orders against each other.

|Pub data |account1: 3, subaccount1: 1, account2: 3, subaccount2: 3, amount12: 3, amount21: 3, fee: 1| 15 bytes|

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

|Pub data |account: 3, subaccount: 1, nonce: 2, fee: 1| 8 bytes|

Comments:
- if nonce in pub data is 0, subaccount nonce is incremented

tbd

# Todo / Questions

- subaccounts for everything (0 is default): transfers from anything to anything?
- sign(amount = 0) authorizes transfer of the entire balance?
- bitmask: explicit control of transfer permission for subaccounts (by Brecht)?
