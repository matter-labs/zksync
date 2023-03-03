-- We want to use the one sequencer for both tables, because most of the time we do sort depends on each other
ALTER TABLE executed_transactions
    ADD COLUMN sequence_number BIGINT;

ALTER TABLE executed_priority_operations
    ADD COLUMN sequence_number BIGINT;

CREATE SEQUENCE executed_operations_seq_number;

ALTER TABLE executed_transactions ALTER COLUMN sequence_number SET DEFAULT nextval('executed_operations_seq_number');
ALTER TABLE executed_priority_operations ALTER COLUMN sequence_number SET DEFAULT nextval('executed_operations_seq_number');
