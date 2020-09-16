ALTER TABLE mempool_txs ADD batch_id bigserial NOT NULL;
ALTER TABLE executed_transactions ADD batch_id BIGINT;
