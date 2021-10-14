CREATE TABLE tx_filters
(
    id BIGSERIAL PRIMARY KEY,
    address bytea NOT NULL,
    token INTEGER NOT NULL,
    tx_hash bytea NOT NULL
);
