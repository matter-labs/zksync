/*
This migration initializes the ZKSync database state by creating all the
required tables and performing the associated operations (e.g. creating indexes).

File is structured to contain statements in the following order:

- Tables creation (split by comments into logically separated sections).
- Indexes creation.
- Extensions enabling.
- Data insertion.

Note that this script does not insert all the required data by itself,
some of the data is inserted by the scripts from the `bin` folder.
To be sure that database is fully initialized, migrations should not
be run directly via `diesel_cli`, but `zksync db-reset` should be used instead.
*/

-- ------------------------------- --
-- Transactions/operations section --
-- ------------------------------- --

-- Table containing all the ZKSync block execution operations.
-- Operations are associated with some block and every block can
-- have multiple related operations with different action types
-- (e.g. `commit` / `verify`).
CREATE TABLE operations (
    id bigserial PRIMARY KEY,
    block_number BIGINT NOT NULL,
    action_type TEXT NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now(),
    confirmed bool NOT NULL DEFAULT false
);

-- Block header entry.
CREATE TABLE blocks (
    number BIGINT PRIMARY KEY,
    root_hash BYTEA NOT NULL,
    fee_account_id BIGINT NOT NULL,
    unprocessed_prior_op_before BIGINT NOT NULL,
    unprocessed_prior_op_after BIGINT NOT NULL,
    block_size BIGINT NOT NULL
);

-- Pending block header entry.
CREATE TABLE pending_block (
    -- Pending block ID
    number BIGINT PRIMARY KEY NOT NULL,
    chunks_left BIGINT NOT NULL,
    unprocessed_priority_op_before BIGINT NOT NULL,
    pending_block_iteration BIGINT NOT NULL
);

-- Table for the executed priority operations (e.g. deposit).
CREATE TABLE executed_priority_operations (
    -- sidechain block info
    block_number BIGINT NOT NULL,
    block_index INT NOT NULL,
    -- operation data
    operation jsonb NOT NULL,
    -- operation metadata
    from_account bytea NOT NULL,
    to_account bytea NOT NULL,
    priority_op_serialid BIGINT NOT NULL,
    deadline_block BIGINT NOT NULL,
    eth_hash bytea NOT NULL,
    eth_block BIGINT NOT NULL,
    created_at timestamp with time zone not null,
    PRIMARY KEY (eth_hash)
);

-- Table for the executed common operations (e.g. transfer).
CREATE TABLE executed_transactions (
    -- sidechain block info
    block_number BIGINT NOT NULL,
    block_index INT,
    -- operation data
    tx jsonb NOT NULL,
    operation jsonb NOT NULL,
    -- operation metadata
    tx_hash bytea NOT NULL,
    from_account bytea NOT NULL,
    to_account bytea,
    success bool NOT NULL,
    fail_reason TEXT,
    primary_account_address bytea NOT NULL,
    nonce BIGINT NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL,
    PRIMARY KEY (tx_hash)
);

-- -------------- --
-- Tokens section --
-- -------------- --

-- Token types known to the ZKSync node.
-- By default has the ETH token only (see the `INSERT` statement in the end of the file).
CREATE TABLE tokens (
    id INTEGER NOT NULL PRIMARY KEY,
    address TEXT NOT NULL,
    symbol TEXT NOT NULL,
    decimals SMALLINT NOT NULL
);

-- ---------------- --
-- Accounts section --
-- ---------------- --

-- Table for the ZKSync accounts.
CREATE TABLE accounts (
    id BIGINT NOT NULL PRIMARY KEY,
    last_block BIGINT NOT NULL,
    nonce BIGINT NOT NULL,
    address bytea NOT NULL,
    pubkey_hash bytea NOT NULL
);

-- Table for the account balance change operations.
CREATE TABLE account_balance_updates (
    balance_update_id serial NOT NULL,
    account_id BIGINT NOT NULL,
    block_number BIGINT NOT NULL,
    coin_id INTEGER NOT NULL REFERENCES tokens(id) ON UPDATE CASCADE,
    old_balance NUMERIC NOT NULL,
    new_balance NUMERIC NOT NULL,
    old_nonce BIGINT NOT NULL,
    new_nonce BIGINT NOT NULL,
    update_order_id INTEGER NOT NULL,
    PRIMARY KEY (balance_update_id)
);

-- Table for the account creation operations.
CREATE TABLE account_creates (
    account_id BIGINT NOT NULL,
    is_create bool NOT NULL,
    block_number BIGINT NOT NULL,
    address bytea NOT NULL,
    nonce BIGINT NOT NULL,
    update_order_id INTEGER NOT NULL,
    PRIMARY KEY (account_id, block_number)
);

-- Table for the account public key change operations.
CREATE TABLE account_pubkey_updates (
    pubkey_update_id serial NOT NULL,
    update_order_id INTEGER NOT NULL,
    account_id BIGINT NOT NULL,
    block_number BIGINT NOT NULL,
    old_pubkey_hash bytea NOT NULL,
    new_pubkey_hash bytea NOT NULL,
    old_nonce BIGINT NOT NULL,
    new_nonce BIGINT NOT NULL,
    PRIMARY KEY (pubkey_update_id)
);

-- Table for the account balances. One account can have several balances,
-- but every balance account has must have an unique token (meaning that
-- there may be user with `ETH` and `ERC-20` balances, but not with `ETH`
-- and `ETH` balances).
CREATE TABLE balances (
    account_id BIGINT REFERENCES accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
    coin_id INTEGER REFERENCES tokens(id) ON UPDATE CASCADE,
    balance NUMERIC NOT NULL DEFAULT 0,
    PRIMARY KEY (account_id, coin_id)
);

-- -------------------- --
-- Data restore section --
-- -------------------- --

CREATE TABLE data_restore_events_state (
    id SERIAL PRIMARY KEY,
    block_type TEXT NOT NULL,
    transaction_hash BYTEA NOT NULL,
    block_num BIGINT NOT NULL
);

CREATE TABLE data_restore_storage_state_update
(
    id SERIAL PRIMARY KEY,
    storage_state TEXT NOT NULL
);

CREATE TABLE data_restore_last_watched_eth_block
(
    id SERIAL PRIMARY KEY,
    block_number TEXT NOT NULL
);

-- Table for the executed franklin operations, used by
CREATE TABLE data_restore_rollup_ops
(
    id SERIAL PRIMARY KEY,
    block_num BIGINT NOT NULL,
    operation JSONB  NOT NULL,
    fee_account BIGINT NOT NULL
);


-- -------------- --
-- Prover section --
-- -------------- --

-- Stored proofs for the blocks.
CREATE TABLE proofs (
    block_number bigserial PRIMARY KEY,
    proof jsonb NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now()
);

-- Ongoing block proving jobs.
CREATE TABLE prover_runs (
    id serial PRIMARY KEY,
    block_number BIGINT NOT NULL,
    worker TEXT,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now(),
    updated_at TIMESTAMP with time zone NOT NULL DEFAULT now()
);

-- List of currently available provers.
CREATE TABLE active_provers (
    id serial PRIMARY KEY,
    worker TEXT NOT NULL,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now(),
    stopped_at TIMESTAMP with time zone,
    block_size BIGINT NOT NULL
);

-- --------------------- --
-- Server config section --
-- --------------------- --

-- Unique server configuration entry.
-- Expected to be initialized separately, e.g. by `zksync db-reset` or `zksync init` command.
CREATE TABLE server_config (
    -- enforce single record
    id bool PRIMARY KEY NOT NULL DEFAULT true,
    CONSTRAINT single_server_config CHECK (id),
    contract_addr TEXT,
    gov_contract_addr TEXT
);

-- ----------- --
-- ETH section --
-- ----------- --

-- Stored Ethereum anchoring operations.
CREATE TABLE eth_operations (
    id bigserial PRIMARY KEY,
    nonce BIGINT NOT NULL,
    confirmed bool NOT NULL DEFAULT false,
    raw_tx bytea NOT NULL,
    op_type TEXT NOT NULL,
    final_hash bytea DEFAULT NULL,
    last_deadline_block BIGINT NOT NULL,
    last_used_gas_price NUMERIC NOT NULL
);

-- Data related to the interaction with the Ethereum network.
CREATE TABLE eth_parameters (
    -- enforce single record
    id bool PRIMARY KEY NOT NULL DEFAULT true,
    -- Locally stored Ethereum nonce
    nonce BIGINT NOT NULL,
    -- Last gas price limit used by GasAdjuster
    gas_price_limit BIGINT NOT NULL,
    -- Gathered operations statistics
    commit_ops BIGINT NOT NULL,
    verify_ops BIGINT NOT NULL,
    withdraw_ops BIGINT NOT NULL
);

-- Table connection `eth_operations` and `operations` table.
-- Each entry provides a mapping between the Ethereum transaction and the ZK Sync operation.
CREATE TABLE eth_ops_binding (
    id bigserial PRIMARY KEY,
    op_id bigserial NOT NULL REFERENCES operations(id),
    eth_op_id bigserial NOT NULL REFERENCES eth_operations(id)
);

-- Table storing all the sent Ethereum transaction hashes.
CREATE TABLE eth_tx_hashes (
    id bigserial PRIMARY KEY,
    eth_op_id bigserial NOT NULL REFERENCES eth_operations(id),
    tx_hash bytea NOT NULL
);

-- --------------- --
-- Mempool section --
-- --------------- --
CREATE TABLE mempool_txs (
    id bigserial PRIMARY KEY,
    -- Hash of the transaction
    tx_hash TEXT NOT NULL,
    -- Transaction contents
    tx jsonb NOT NULL
);

-- --------------- --
-- Indexes section --
-- --------------- --

-- Indexes are built for tables on the columns which are used intensively in queries.
CREATE INDEX operations_block_index ON operations (block_number);
CREATE INDEX blocks_root_hash_index ON blocks (root_hash);
CREATE INDEX tokens_symbol_index ON tokens (symbol);
CREATE INDEX eth_ops_binding_op_id_index ON eth_ops_binding (op_id);
CREATE INDEX eth_tx_hashes_eth_op_id_index ON eth_tx_hashes (eth_op_id);
CREATE INDEX mempool_txs_hash_index ON mempool_txs (tx_hash);

CREATE INDEX accounts_block_index ON accounts (last_block);
CREATE INDEX accounts_address_index ON accounts (address);
CREATE INDEX account_balance_updates_block_index ON account_balance_updates (block_number);
CREATE INDEX account_creates_block_index ON account_creates (block_number);
CREATE INDEX account_pubkey_updates_block_index ON account_pubkey_updates (block_number);

CREATE INDEX executed_transactions_block_number_index ON executed_transactions (block_number);
CREATE INDEX executed_transactions_hash_index ON executed_transactions (tx_hash);
CREATE INDEX executed_transactions_from_account_index ON executed_transactions (from_account);
CREATE INDEX executed_transactions_to_account_index ON executed_transactions (to_account);

CREATE INDEX executed_priority_operations_block_index ON executed_priority_operations (block_number);
CREATE INDEX executed_priority_operations_serialid_index ON executed_priority_operations (priority_op_serialid);
CREATE INDEX executed_priority_operations_eth_hash_index ON executed_priority_operations (eth_hash);
CREATE INDEX executed_priority_operations_from_account_index ON executed_priority_operations (from_account);
CREATE INDEX executed_priority_operations_to_account_index ON executed_priority_operations (to_account);

-- ------------------ --
-- Extensions section --
-- ------------------ --

-- `tablefunc` enables `crosstab` (pivot)
CREATE EXTENSION IF NOT EXISTS tablefunc;

-- ---------------------- --
-- Data insertion section --
-- ---------------------- --

-- Add ETH token
INSERT INTO tokens
VALUES (0,
        '0x0000000000000000000000000000000000000000',
        'ETH',
        18);
