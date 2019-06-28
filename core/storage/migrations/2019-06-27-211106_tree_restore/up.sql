-- Your SQL goes here
CREATE TABLE tree_restore_network (
    id SERIAL PRIMARY KEY,
    network_id SMALLINT NOT NULL
);

CREATE TABLE tree_restore_last_watched_eth_block (
    id SERIAL PRIMARY KEY,
    block_number TEXT NOT NULL
);

CREATE TABLE block_events (
    id SERIAL PRIMARY KEY,
    block_type TEXT NOT NULL,
    transaction_hash BYTEA NOT NULL,
    block_num BIGINT NOT NULL
);

CREATE TABLE franklin_transactions (
    id SERIAL PRIMARY KEY,
    franklin_transaction_type TEXT NOT NULL,
    block_number BIGINT NOT NULL,
    eth_tx_hash BYTEA NOT NULL,
    eth_tx_nonce TEXT NOT NULL,
    eth_tx_block_hash BYTEA,
    eth_tx_block_number TEXT,
    eth_tx_transaction_index TEXT,
    eth_tx_from BYTEA NOT NULL,
    eth_tx_to BYTEA,
    eth_tx_value TEXT NOT NULL,
    eth_tx_gas_price TEXT NOT NULL,
    eth_tx_gas TEXT NOT NULL,
    eth_tx_input BYTEA NOT NULL,
    commitment_data BYTEA NOT NULL
);
