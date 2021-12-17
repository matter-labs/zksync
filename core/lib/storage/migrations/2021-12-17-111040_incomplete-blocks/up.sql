-- Incomplete block (one that does not yet have a root hash calculated) header entry.
-- It mimics the `block` table with the only exception that it does not have the block root hash in it.
CREATE TABLE IF NOT EXISTS incomplete_blocks (
    number BIGINT PRIMARY KEY,
    fee_account_id BIGINT NOT NULL,
    unprocessed_prior_op_before BIGINT NOT NULL,
    unprocessed_prior_op_after BIGINT NOT NULL,
    block_size BIGINT NOT NULL,
    commit_gas_limit BIGINT NOT NULL,
    verify_gas_limit BIGINT NOT NULL,
    timestamp bigint,
    commitment BYTEA NOT NULL default '\x0000000000000000000000000000000000000000000000000000000000000000',
);
