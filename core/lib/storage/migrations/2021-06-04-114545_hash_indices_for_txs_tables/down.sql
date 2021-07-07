CREATE INDEX IF NOT EXISTS executed_transactions_hash_index
    ON "executed_transactions" USING btree (tx_hash);
DROP INDEX IF EXISTS executed_transactions_tx_hash_idx;

CREATE INDEX IF NOT EXISTS executed_priority_operations_from_account_index 
    ON "executed_priority_operations" USING btree (from_account);
CREATE INDEX IF NOT EXISTS executed_priority_operations_to_account_index
    ON "executed_priority_operations" USING btree (to_account);
CREATE INDEX IF NOT EXISTS executed_priority_operations_eth_hash_index
    ON "executed_priority_operations" USING btree (eth_hash);
DROP INDEX IF EXISTS executed_priority_operations_from_account_idx;
DROP INDEX IF EXISTS executed_priority_operations_to_account_idx;
DROP INDEX IF EXISTS executed_priority_operations_eth_hash_idx;

CREATE INDEX IF NOT EXISTS mempool_txs_hash_index
    ON "mempool_txs" USING btree (tx_hash);
DROP INDEX IF EXISTS mempool_txs_tx_hash_idx;
