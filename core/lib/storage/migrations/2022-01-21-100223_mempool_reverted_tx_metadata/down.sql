DROP TABLE mempool_reverted_txs_meta;
DROP TABLE reverted_block;
DROP TYPE operation_type;
ALTER TABLE mempool_txs DROP COLUMN reverted;
