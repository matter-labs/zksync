ALTER TABLE mempool_txs DROP COLUMN created_at;
ALTER TABLE mempool_txs DROP COLUMN eth_sign_data;
ALTER TABLE executed_transactions DROP COLUMN eth_sign_data;
