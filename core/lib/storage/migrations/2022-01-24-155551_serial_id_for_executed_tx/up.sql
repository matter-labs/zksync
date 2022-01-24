-- We want to use the one sequencer for both tables, because most of the time we do sort depends on each other
ALTER TABLE executed_transactions
    ADD COLUMN sequencer_id BIGINT;

ALTER TABLE executed_priority_operations
    ADD COLUMN sequencer_id BIGINT;

CREATE SEQUENCE executed_operations_id_seq;

ALTER TABLE executed_transactions ALTER COLUMN sequencer_id SET DEFAULT nextval('executed_operations_id_seq');
ALTER TABLE executed_priority_operations ALTER COLUMN sequencer_id SET DEFAULT nextval('executed_operations_id_seq');


-- TODO update sequencer_id