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

-- Table connection `eth_operations` and `operations` table.
-- Each entry provides a mapping between the Ethereum transaction and the ZK Sync operation.
CREATE TABLE eth_ops_binding
(
    id             bigserial PRIMARY KEY,
    op_id          bigserial NOT NULL REFERENCES operations (id),
    eth_op_id      bigserial NOT NULL REFERENCES eth_operations (id)
);

ALTER TABLE eth_operations
    -- Add the operation type (`commit` / `verify` / `withdraw`).
    ADD COLUMN op_type text not null,
    -- Remove the `op_id` field, since `withdraw` operation does not have an associated operation.
    -- The `eth_ops_binding` table should be used since now.
    DROP COLUMN op_id CASCADE,
    -- Rename `gas_price` to `last_used_gas_price`, since it's the only field changed for resent txs
    -- and it makes no sense to store every sent transaction separately.
    DROP COLUMN gas_price CASCADE,
    ADD COLUMN last_used_gas_price numeric not null
