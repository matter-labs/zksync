DROP TABLE IF EXISTS data_restore_rollup_block_ops;
TRUNCATE data_restore_rollup_blocks;
ALTER TABLE data_restore_rollup_blocks ADD COLUMN operation JSONB NOT NULL;
ALTER TABLE data_restore_rollup_blocks DROP CONSTRAINT IF EXISTS data_restore_rollup_blocks_pkey;
ALTER TABLE data_restore_rollup_blocks ADD COLUMN id SERIAL PRIMARY KEY;
ALTER TABLE data_restore_rollup_blocks RENAME TO data_restore_rollup_ops;
