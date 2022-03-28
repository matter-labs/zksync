CREATE TABLE IF NOT EXISTS tx_filters
(
    address bytea NOT NULL,
    token INTEGER NOT NULL,
    tx_hash bytea NOT NULL,
    PRIMARY KEY (address, token, tx_hash)
);
CREATE INDEX IF NOT EXISTS tx_filters_address_idx ON "tx_filters" USING hash (address);
