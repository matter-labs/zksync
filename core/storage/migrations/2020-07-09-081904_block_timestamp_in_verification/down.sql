ALTER TABLE data_restore_rollup_ops DROP COLUMN block_timestamp;
ALTER TABLE blocks DROP COLUMN block_timestamp;
ALTER TABLE pending_block DROP COLUMN  block_timestamp;
