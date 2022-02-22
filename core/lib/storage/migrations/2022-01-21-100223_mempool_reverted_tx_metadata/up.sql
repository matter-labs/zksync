CREATE TYPE operation_type AS ENUM ('L1', 'L2');

CREATE TABLE IF NOT EXISTS mempool_reverted_txs_meta (
   block_number BIGINT NOT NULL,
   block_index INT,
   tx_type operation_type not null,
   operation jsonb NOT NULL,
   tx_hash TEXT NOT NULL,
   tx_hash_bytes bytea NOT NULL,
   nonce bigint,
   from_account bytea not null,
   to_account bytea,
   success bool NOT NULL,
   fail_reason TEXT,
   primary_account_address bytea NOT NULL,
   PRIMARY KEY (tx_hash)
);

CREATE TABLE IF NOT EXISTS reverted_block (
    number BIGINT PRIMARY KEY ,
    unprocessed_priority_op_before BIGINT NOT NULL,
    unprocessed_priority_op_after BIGINT NOT NULL,
    timestamp BIGINT NOT NULL
);

ALTER TABLE mempool_txs ADD COLUMN reverted BOOL NOT NULL DEFAULT FALSE;
