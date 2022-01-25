-- We want to use the one sequencer for both tables, because most of the time we do sort depends on each other
ALTER TABLE executed_transactions
    ADD COLUMN sequence_number BIGINT;

ALTER TABLE executed_priority_operations
    ADD COLUMN sequence_number BIGINT;

CREATE SEQUENCE executed_operations_seq_number;

-- After this manipulations the new transactions will be added with correct seq numbers
BEGIN;

-- The number of transactions in this table is guaranteed to exceed the number of transactions in both executed_transactions and executed_priority_operations.
LOCK TABLE tx_filters IN SHARE MODE;
SELECT setval(executed_operations_seq_number, (SELECT count(*) FROM tx_filters )+1);
ALTER TABLE executed_transactions ALTER COLUMN sequence_number SET DEFAULT nextval('executed_operations_seq_number');
ALTER TABLE executed_priority_operations ALTER COLUMN sequence_number SET DEFAULT nextval('executed_operations_seq_number');

COMMIT;
