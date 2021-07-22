CREATE TABLE txs_batches_signatures (
    batch_id BIGINT PRIMARY KEY,
    eth_signature JSONB NOT NULL
);
