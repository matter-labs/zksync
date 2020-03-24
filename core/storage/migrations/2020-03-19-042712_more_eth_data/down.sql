-- This file should undo anything in `up.sql`
DROP TABLE IF EXISTS eth_nonce CASCADE;
DROP TABLE IF EXISTS eth_stats CASCADE;
DROP TABLE IF EXISTS eth_ops_binding CASCADE;
DROP TABLE IF EXISTS eth_tx_hashes CASCADE;

ALTER TABLE eth_operations
    ADD COLUMN op_id bigserial REFERENCES operations (id),
    DROP COLUMN op_type CASCADE,
    ADD COLUMN gas_price numeric not null,
    DROP COLUMN last_used_gas_price CASCADE