ALTER TABLE executed_priority_operations
    ADD tx_hash bytea NOT NULL;
ALTER TABLE executed_priority_operations
    ADD eth_block_index bigint NOT NULL;