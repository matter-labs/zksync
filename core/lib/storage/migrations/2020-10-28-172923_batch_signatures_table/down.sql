DROP TRIGGER IF EXISTS delete_signatures ON mempool_txs;
DROP FUNCTION IF EXISTS delete_signatures();
DROP TABLE IF EXISTS mempool_batches_signatures;
