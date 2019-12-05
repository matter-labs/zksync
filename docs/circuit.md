# ZKSync circuit
Circuit describes R1CS, for further theoretical information look here: https://github.com/matter-labs/awesome-zero-knowledge-proofs

## State structure

- 2^24 accounts
- 2^10 balance leafs under each account: 

Full Merkle tree height: 34.

<img src="https://docs.google.com/drawings/d/e/2PACX-1vQmABflC3CUHQb62x6fDmyLnVFQbqZGAoJW8j9T6WSKL-ixOtU3xLjd2_hJRCRVn2fTq17Bs1ySQUbj/pub?h=520">

([edit diagram here](https://docs.google.com/drawings/d/13bFjrSipx8-RKyAPbxzCCyXtswzvuFLjD-O8QEYaUYA/edit?usp=sharing))

Token type is determined by the index in the balance tree.

## Circuit overview

Currently our circuit is working over BN256 curve, its operations are implemented here: https://github.com/matter-labs/bellman
As a gadget lib we use: https://github.com/matter-labs/franklin-crypto
The only one public input to circuit is `public_data_commitment` which is the one from contract.

What circuit basically does is cycles over `BLOCK_CHUNKS_SIZE` chunks and applies operation to state one by one (each transaction, 
such as transfer, deposit, etc. consists of some amount of operations, see below). 
### Operation Processing
Each operation is divided into chunks (corresponding to `pubdata_chunks`, number of chunks can be seen in the [table](https://docs.google.com/spreadsheets/d/1ejK1MJfVehcwjgjVDFD3E2k1EZ7auqbG_y0DKidS9nA/edit?usp=drive_open&ouid=102923468016872611309)). 
Each chunk has its own part of the public data (8 bytes) given through witness, together with needed `audit_pathes` and other info to prove state. 

#### Chunk processing

Operation processing does three main things:
1. Aggregates public data provided in witness (and proves that this public data is corresponding to operation arguments as described in the [table](https://docs.google.com/spreadsheets/d/1ejK1MJfVehcwjgjVDFD3E2k1EZ7auqbG_y0DKidS9nA/edit?usp=drive_open&ouid=102923468016872611309))
2. Signature is verified 
3. State changing: During processing of one chunk we only update one leaf of the tree (which consists of calculating `root_hash` before applying, proving it is 
equal to the previous chunk's `root_hash` and calculating `root_hash` after applying). Current convention is that if operation changes state of 
only one leaf -- than it is done in the first chunk, for `transfer` and `transfer_to_new` first operation updates sender's leaf, second operation updates
receiver's leaf, all other operations preserve state (`root_hash` is not changed).
4. Aggregating fees 

After all chunks are processed (which is the end of main cycle if you look into the code), we first add aggregated fees to the account with id `validator_account_id`. Then all `public_data` aggregated during chunk processing is 
used to calculate `public_data_commitment` and proving it is equal to public input.

#### Public data commitment computation
```
h1 = sha256(block_number || operator_account_id)
h2 = sha256(h1 || previous_root)
h3 = sha256(h2 || new_root)
public_data_commitment = sha256(h3 || public_data)
```

This commitment ensures that in some block with `block_number` we made operations which are described in `public_data`, fees are withheld to
`operator_account_id` and this changed state in the way that `root_hash` of the whole merkle tree migrated from `previous_root` to `new_root`



## Pub data 
### Overview
Spec for the exact bytes of `pub_data` for each operation is given in another document (TODO: actually create one)
Spec for the exact bytes of `signatures` for each is given in another document (TODO: actually create one)

Pub data is chosen in the way that you have all information needed to reconstruct state.

### Structure

Public data is represented by a byte string with concatenated pub data of the transaction. Each pub data set is prefixed with a single byte of the type of operation.

|tx1_optype|tx1_pub_data|tx2_optype|tx2_pub_data|...

Public data of each operation is padded to the maximum pub data size of the circuit.

The size of pub data members is indicated in bytes. Pubkeys in pub data are represented by Pedersen hashes truncated to 160 bits.

### By operation

([See table here](https://docs.google.com/spreadsheets/d/1ejK1MJfVehcwjgjVDFD3E2k1EZ7auqbG_y0DKidS9nA/edit?usp=drive_open&ouid=102923468016872611309))

## Main circuit ops

Each operation in the circuit requires:
- 2 Merkle path proof checks of height 34
- 1 signature check

### Circuit operations

#### noop (optype = 0)

Phony operation for block padding.

Comments:
- Optype must equal 0 so that padding can efficiently be added to the pub data before hashing in the smart contract.

#### deposit

Create an account and deposit a balance into it.

Verification:
- User initiates a deposit by a transaction on the root chain which creates a deposit queue entry
- Public data for the transaction is checked by the smart contract against the deposit queue

#### transfer_to_new

Create a new account and transfer some balance into it from an existing one.

Verification:
- Owner of existing account signs (optype, account, token, nonce, amount, fee, pubkey_hash)

Comments:
- Splitting into `transfer_to_new` and `deposit_from` operations is necessary because for each operation in this circuit we only update one account/balance leaf in the tree.

#### withdraw

Withdraw part of a particular token balance to the mainchain.

Verification:
- Account owner signs (optype, account, token, leaf_nonce, amount, fee)

#### transfer

Transfer an amount of tokens from one account balance to another.

|Pub data|Total size|
|--------|----------|
|amount: 3, token: 1, from_account: 3, to_account: 3, fee: 1| 11 bytes|

Verification:
- Account owner signs (optype, from_account, to_account, token, amount, fee, nonce)


# Todo / Questions

- verify whether musig_pedersen is valid signature that we can use instead of musig_sha256