ALTER TABLE executed_priority_operations
    ADD eth_block_index bigint;

ALTER TABLE executed_priority_operations
    ADD tx_hash bytea NOT NULL DEFAULT ''::bytea;

CREATE TABLE txs_batches_hashes (
    batch_id BIGSERIAL PRIMARY KEY,
    batch_hash bytea NOT NULL
);
