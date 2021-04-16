--This migration is made for tests passing in CI.
--These migration should be deleted and should be done manually.
ALTER TABLE executed_priority_operations
    ADD eth_block_index bigint;
ALTER TABLE executed_priority_operations
    ADD tx_hash bytea NOT NULL;
