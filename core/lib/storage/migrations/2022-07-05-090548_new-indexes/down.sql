DROP INDEX IF EXISTS ix_tx_filters_tx_hash_address;
CREATE INDEX IF NOT EXISTS tokens_symbol_index on public.tokens (symbol);
DROP INDEX IF EXISTS tokens_symbol_lower_idx;
DROP INDEX IF EXISTS ix_executed_transactions_failed_at;
CREATE INDEX IF NOT EXISTS executed_transactions_tx_hash_idx
    ON "executed_transactions" USING hash (tx_hash);
DROP INDEX IF EXISTS ix_prover_job_queue_job_type_last_block;
DROP INDEX IF EXISTS aggregate_operations_action_type_to_block_true_idx;
DROP INDEX IF EXISTS aggregate_operations_action_type_to_block_false_idx;
DROP INDEX IF EXISTS aggregate_operations_action_type_to_block_idx;
DROP INDEX IF EXISTS ix_prover_job_queue_job_status_updated_at ;
