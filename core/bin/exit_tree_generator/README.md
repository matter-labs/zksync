# Exit Tree Generator

This tool is designed to help users withdraw their tokens from zkSync Lite using an exit tree. It provides functionality
to restore the zkSync Merkle tree state, generate new leaves for the exit tree, calculate Merkle roots, and create
proofs for claiming funds.

## Overview

The exit tree generator processes account and balance data from zkSync Lite to create a Merkle tree that enables users
to claim their funds through a smart contract. The tool primarily supports simple workflows for users who just need to
generate proofs, with additional advanced workflows available for full tree restoration and verification.

## Input Files

MatterLabs provides the following CSV files containing the state of zkSync Lite at the last verified block:

### 1. `accounts.csv`

Contains the account state for the last verified block. Each row includes:

- Account ID
- Nonce
- Ethereum address
- Public key hash

### 2. `balances.csv`

Contains the balance information for each account and token combination. Each row includes:

- Account ID
- Token ID (coin ID)
- Balance amount

### 3. `tokens.csv`

Maps token IDs to their Ethereum token addresses. This file includes both fungible and non-fungible tokens.
**Important:** You must use the `tokens.csv` file provided by MatterLabs when creating new leaves, as it contains NFT
addresses that are required for proper tree generation. For non-fungible tokens, you can verify token IDs using the
`restore-token-ids` command.

## Usage

The tool provides two usage modes: **Regular** (for simple proof generation) and **Advanced** (for full tree restoration
and verification).

### Regular Usage

For regular users who just need to generate proofs from an existing `new_leaves.csv` file:

#### Calculate Merkle Root

```bash
cargo run --bin zksync_exit_tree_generator -- calculate-root-for-keccak-tree \
    [new_leaves.csv]
```

#### Create Proof

```bash
cargo run --bin zksync_exit_tree_generator -- create-proof \
    --account <ACCOUNT_ADDRESS> \
    --tokens <TOKEN_ADDRESS_1> [<TOKEN_ADDRESS_2> ...] \
    [new_leaves.csv]
```

You can provide multiple token addresses to create a proof for multiple tokens at once. The proof will be printed as a
hex-encoded string that can be used with the withdrawal contract.

### Advanced Usage

For advanced users who want to restore the entire tree and verify the process:

#### Step 1: Restore Token IDs (Optional)

If you need to verify or restore token IDs for non-fungible tokens:

```bash
cargo run --bin zksync_exit_tree_generator -- restore-token-ids \
    --web3 <WEB3_URL> \
    --config <CONFIG_PATH>
```

This will restore token IDs and save them to `restored_tokens.csv`.

#### Step 2: Restore the Tree

Restore the root hash of the latest block on zkSync Lite from the provided CSV files:

```bash
cargo run --bin zksync_exit_tree_generator -- restore-zksync-tree \
    --accounts accounts.csv \
    --balances balances.csv
```

**Note:** This operation takes approximately 6 hours to complete.

The command will output the restored tree root hash, which you can validate against the zkSync contract.

#### Step 3: Create New Leaves

Generate new leaves for the exit tree:

```bash
cargo run --bin zksync_exit_tree_generator -- create-new-leaves \
    --accounts accounts.csv \
    --balances balances.csv \
    --tokens tokens.csv \
    [--output new_leaves.csv]
```

**Important:** You must use the `tokens.csv` file provided by MatterLabs for this step, as it contains NFT addresses
that are essential for generating the correct leaves.

This creates a file (default: `new_leaves.csv`) that contains the leaves needed to calculate the Merkle root and Merkle
paths for the new tree.

#### Step 4: Calculate and Verify Merkle Root

Calculate the Merkle root for the Keccak tree:

```bash
cargo run --bin zksync_exit_tree_generator -- calculate-root-for-keccak-tree \
    [new_leaves.csv]
```

This root will be published on-chain and can be used to verify the tree integrity.

#### Step 5: Create Proof for Claiming Funds

Generate a Merkle proof for a specific account and one or more tokens:

```bash
cargo run --bin zksync_exit_tree_generator -- create-proof \
    --account <ACCOUNT_ADDRESS> \
    --tokens <TOKEN_ADDRESS_1> [<TOKEN_ADDRESS_2> ...] \
    [new_leaves.csv]
```

You can provide multiple token addresses to create a proof for multiple tokens at once. The proof can then be sent to
the smart contract to withdraw funds.

## Commands Reference

### `restore-token-ids`

Restores token IDs by querying the Ethereum blockchain.

**Options:**

- `--web3 <WEB3_URL>`: Web3 API URL (optional, uses env config if not provided)
- `--config <CONFIG_PATH>`: Path to configuration file (optional, uses env config if not provided)

**Output:** `restored_tokens.csv`

### `restore-zksync-tree`

Restores the zkSync Merkle tree from CSV files and calculates the root hash.

**Options:**

- `--accounts <PATH>`: Path to accounts CSV file (required)
- `--balances <PATH>`: Path to balances CSV file (required)

**Output:** Prints the restored tree root hash

### `create-new-leaves`

Creates new leaves for the exit Merkle tree.

**Options:**

- `--accounts <PATH>`: Path to accounts CSV file (required)
- `--balances <PATH>`: Path to balances CSV file (required)
- `--tokens <PATH>`: Path to tokens CSV file (required) - **Must use the tokens.csv provided by MatterLabs as it
  contains NFT addresses**
- `--output <PATH>`: Optional output file path (default: `new_leaves.csv`)

**Output:** CSV file with Merkle tree leaves

### `calculate-root-for-keccak-tree`

Calculates the Merkle root hash from the leaves file.

**Arguments:**

- `<LEAVES_PATH>`: Path to leaves CSV file (optional, default: `new_leaves.csv`)

**Output:** Prints the calculated Merkle root hash

### `create-proof`

Generates a Merkle proof for a specific account and one or more tokens.

**Options:**

- `--account <ADDRESS>`: Ethereum address of the account (required)
- `--tokens <ADDRESS>...`: Ethereum address(es) of the token(s) (required, can specify multiple)
- `<LEAVES_PATH>`: Path to leaves CSV file (optional, default: `new_leaves.csv`)

**Output:** Prints the hex-encoded Merkle proof

**Note:** You can provide multiple token addresses to create a proof for multiple tokens in a single command.

### `restore-tree-from-db` (PostgreSQL feature)

Restores the Merkle tree directly from the verified database state. Requires the `postgres` feature flag. DATABASE_URL
environment variable must be set to connect to the PostgreSQL database.

```bash
cargo run --features postgres --bin zksync_exit_tree_generator -- restore-tree-from-db
```

## Building

Build the tool:

```bash
cargo build --release --bin zksync_exit_tree_generator
```

For PostgreSQL support:

```bash
cargo build --release --features postgres --bin zksync_exit_tree_generator
```

## Output Files

- `new_leaves.csv`: Default output file containing Merkle tree leaves
- `restored_tokens.csv`: Output file from token ID restoration
- `internals.txt`: Internal node hashes for speeding up further recalculation

## Notes

- The `restore-zksync-tree` operation is computationally intensive and takes approximately 6 hours
- All addresses should be provided in standard Ethereum hex format (0x...)
- Token addresses in `tokens.csv` includes both fungible and non-fungible tokens. But only fungible tokens are restored
  from the blockchain in the `restore-token-ids` command.
