CREATE TABLE tx_filters
(
    address bytea NOT NULL,
    token INTEGER NOT NULL,
    tx_hash bytea NOT NULL,
    PRIMARY KEY (address, token, tx_hash)
);
