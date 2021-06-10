DROP INDEX IF EXISTS executed_transactions_hash_index;
CREATE INDEX IF NOT EXISTS executed_transactions_tx_hash_idx
    ON "executed_transactions" USING hash (tx_hash);

DROP INDEX IF EXISTS executed_priority_operations_from_account_index;
DROP INDEX IF EXISTS executed_priority_operations_to_account_index;
DROP INDEX IF EXISTS executed_priority_operations_eth_hash_index;
CREATE INDEX IF NOT EXISTS executed_priority_operations_from_account_idx 
    ON "executed_priority_operations" USING hash (from_account);
CREATE INDEX IF NOT EXISTS executed_priority_operations_to_account_idx
    ON "executed_priority_operations" USING hash (to_account);
CREATE INDEX IF NOT EXISTS executed_priority_operations_eth_hash_idx
    ON "executed_priority_operations" USING hash (eth_hash);

DROP INDEX IF EXISTS mempool_txs_hash_index;
CREATE INDEX IF NOT EXISTS mempool_txs_tx_hash_idx
    ON "mempool_txs" USING hash (tx_hash);
