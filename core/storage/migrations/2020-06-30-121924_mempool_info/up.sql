ALTER TABLE mempool_txs ADD COLUMN created_at TIMESTAMP with time zone NOT NULL DEFAULT NOW();
