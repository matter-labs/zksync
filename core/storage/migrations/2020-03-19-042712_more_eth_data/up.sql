-- Your SQL goes here
-- Locally stored Ethereum nonce
CREATE TABLE eth_nonce (
    -- enforce single record
    id              bool PRIMARY KEY NOT NULL DEFAULT true,
    nonce           BIGINT NOT NULL
);

-- Gathered operations statistics
CREATE TABLE eth_stats (
    -- enforce single record
    id              bool PRIMARY KEY NOT NULL DEFAULT true,
    commit_ops      BIGINT NOT NULL,
    verify_ops      BIGINT NOT NULL,
    withdraw_ops    BIGINT NOT NULL
);
