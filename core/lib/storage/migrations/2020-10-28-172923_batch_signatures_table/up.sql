CREATE TABLE mempool_batches_signatures (
    batch_id bigserial PRIMARY KEY,
    eth_signature JSONB NOT NULL
);
