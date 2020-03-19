-- Your SQL goes here
CREATE TABLE eth_nonce (
    -- enforce single record
    id              bool PRIMARY KEY NOT NULL DEFAULT true,
    nonce           BIGINT NOT NULL
);
