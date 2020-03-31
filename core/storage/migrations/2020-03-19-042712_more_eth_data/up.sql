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

-- Table storing all the sent Ethereum transaction hashes.
CREATE TABLE eth_tx_hashes
(
    id             bigserial PRIMARY KEY,
    eth_op_id      bigserial NOT NULL REFERENCES eth_operations (id),
    tx_hash        bytea     NOT NULL
);

ALTER TABLE eth_operations
    -- Add the operation type (`commit` / `verify` / `withdraw`).
    ADD COLUMN op_type text not null,
    -- Remove the `op_id` field, since `withdraw` operation does not have an associated operation.
    -- The `eth_ops_binding` table should be used since now.
    DROP COLUMN op_id CASCADE,
    -- Different tx hashes are now stored in the `eth_tx_hashes` table, so this field isn't needed anymore.
    DROP COLUMN tx_hash CASCADE,
    -- Add the field containing the final hash of the committed tx.
    -- This field is `null` until tx has enough confirmations.
    ADD COLUMN final_hash bytea default null,
    -- Rename `deadline_block` to `last_deadline_block`
    DROP COLUMN deadline_block CASCADE,
    ADD COLUMN last_deadline_block bigint not null,
    -- Rename `gas_price` to `last_used_gas_price`
    DROP COLUMN gas_price CASCADE,
    ADD COLUMN last_used_gas_price numeric not null
