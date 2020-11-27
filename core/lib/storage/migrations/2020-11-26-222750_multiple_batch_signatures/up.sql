ALTER TABLE txs_batches_signatures DROP CONSTRAINT txs_batches_signatures_pkey;
ALTER TABLE txs_batches_signatures ADD COLUMN id SERIAL PRIMARY KEY;
