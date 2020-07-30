ALTER TABLE mempool_txs ADD COLUMN created_at TIMESTAMP with time zone NOT NULL DEFAULT NOW();
ALTER TABLE mempool_txs ADD COLUMN eth_sign_data JSONB;
ALTER TABLE executed_transactions ADD COLUMN eth_sign_data JSONB;
