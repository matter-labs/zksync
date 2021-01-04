-- This might not help as long as batch_id is not necessarily unique which
-- makes this change not completely revertible.
ALTER TABLE txs_batches_signatures DROP COLUMN id;
ALTER TABLE txs_batches_signatures ADD PRIMARY KEY (batch_id);
