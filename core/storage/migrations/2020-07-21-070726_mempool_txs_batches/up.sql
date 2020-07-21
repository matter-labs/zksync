CREATE TABLE mempool_batch_binding (
    id bigserial PRIMARY KEY,
    batch_id bigserial NOT NULL,
    mempool_tx_id bigserial NOT NULL REFERENCES mempool_txs(id)
);
