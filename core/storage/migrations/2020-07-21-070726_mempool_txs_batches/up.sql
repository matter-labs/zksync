CREATE TABLE mempool_batches (
    id bigserial PRIMARY KEY
);

CREATE TABLE mempool_batch_binding (
    id bigserial PRIMARY KEY,
    batch_id bigserial NOT NULL REFERENCES mempool_batches(id) ON DELETE CASCADE,
    mempool_tx_id bigserial NOT NULL REFERENCES mempool_txs(id)
);
