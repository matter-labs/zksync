DROP TABLE IF EXISTS eth_last_known_timestamp;
ALTER TABLE data_restore_rollup_ops DROP COLUMN block_timestamp;
ALTER TABLE blocks DROP COLUMN block_timestamp;
