-- Your SQL goes here
CREATE TABLE eth_stats (
    -- enforce single record
    id              bool PRIMARY KEY NOT NULL DEFAULT true,
    commit_ops      BIGINT NOT NULL,
    verify_ops      BIGINT NOT NULL,
    withdraw_ops    BIGINT NOT NULL
);
