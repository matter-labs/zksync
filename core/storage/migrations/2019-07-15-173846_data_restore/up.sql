-- Your SQL goes here
CREATE TABLE data_restore_last_watched_eth_block (
    id SERIAL PRIMARY KEY,
    block_number TEXT NOT NULL
);

CREATE TABLE events_state (
    id SERIAL PRIMARY KEY,
    block_type TEXT NOT NULL,
    transaction_hash BYTEA NOT NULL,
    block_num BIGINT NOT NULL,
    fee_account BIGINT NOT NULL
);

CREATE TABLE franklin_ops (
    id SERIAL PRIMARY KEY,
    block_num BIGINT NOT NULL,
    operation JSONB NOT NULL,
    fee_account BIGINT NOT NULL
);

CREATE TABLE storage_state_update (
    id SERIAL PRIMARY KEY,
    storage_state TEXT NOT NULL
);
