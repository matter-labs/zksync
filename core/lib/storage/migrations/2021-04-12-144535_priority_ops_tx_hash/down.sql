ALTER TABLE executed_priority_operations
    DROP COLUMN IF EXISTS eth_block_index;
ALTER TABLE executed_priority_operations
    DROP COLUMN IF EXISTS tx_hash;
DROP TABLE IF EXISTS txs_batches_hashes;
