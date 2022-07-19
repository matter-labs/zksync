ALTER TABLE tx_filters ADD COLUMN sequence_number BIGINT;
ALTER TABLE tx_filters ADD COLUMN is_priority bool;

CREATE UNIQUE INDEX IF NOT EXISTS uq_executed_transactions_sequence_number ON public.executed_transactions USING btree (sequence_number);
DROP INDEX IF EXISTS executed_transactions_sequence_number;
DROP INDEX IF EXISTS ix_executed_transactions_tx_hash_sequence_number;

CREATE UNIQUE INDEX IF NOT EXISTS uq_executed_priority_operations_sequence_number ON public.executed_priority_operations USING btree (sequence_number);
DROP INDEX IF EXISTS executed_priority_operations_sequence_number;
DROP INDEX IF EXISTS ix_executed_priority_operations_tx_hash_sequence_number;

CREATE INDEX IF NOT EXISTS ix_tx_filters_address_sequence_number ON public.tx_filters USING btree (address, sequence_number) include(is_priority);
DROP INDEX IF EXISTS ix_tx_filters_address_tx_hash;
