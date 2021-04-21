ALTER TABLE executed_priority_operations
    ADD eth_block_index bigint;
ALTER TABLE executed_priority_operations
    ADD tx_hash bytea NOT NULL DEFAULT ''::bytea;
CREATE TABLE txs_batches_hashes (
    batch_id BIGSERIAL PRIMARY KEY,
    tx_batch_hash bytea NOT NULL
);
-- Also you need to calculate tx_hashes for existing operation and batches.
-- You should run `calculate_priority_op_tx_hashes` for this.