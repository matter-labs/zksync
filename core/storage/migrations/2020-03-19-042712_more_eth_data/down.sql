-- This file should undo anything in `up.sql`
DROP TABLE IF EXISTS eth_nonce CASCADE;
DROP TABLE IF EXISTS eth_stats CASCADE;
DROP TABLE IF EXISTS eth_ops_binding CASCADE;
DROP TABLE IF EXISTS eth_tx_hashes CASCADE;

ALTER TABLE eth_operations
    -- Restore `op_id`
    ADD COLUMN op_id bigserial REFERENCES operations (id),
    -- Restore `tx_hash` field
    ADD COLUMN tx_hash bytea not null,
    -- Remove `op_type`
    DROP COLUMN op_type CASCADE,
    -- Rename `last_used_gas_price` to `gas_price`
    ADD COLUMN gas_price numeric not null,
    DROP COLUMN last_used_gas_price CASCADE,
    -- Rename `last_deadline_block` to `deadline_block`
    ADD COLUMN deadline_block bigint not null,
    DROP COLUMN last_deadline_block CASCADE,
    -- Remove `final_hash`
    DROP COLUMN final_hash CASCADE
