-- Extract operations from blocks into a separate relation
-- and rename these tables respectively.
TRUNCATE data_restore_rollup_ops;
ALTER TABLE data_restore_rollup_ops RENAME TO data_restore_rollup_blocks;
ALTER TABLE data_restore_rollup_blocks DROP COLUMN operation, DROP COLUMN id;
ALTER TABLE data_restore_rollup_blocks ADD PRIMARY KEY (block_num);

CREATE TABLE data_restore_rollup_block_ops
(
    id SERIAL PRIMARY KEY,
    block_num BIGINT NOT NULL REFERENCES data_restore_rollup_blocks(block_num) ON DELETE CASCADE,
    operation JSONB  NOT NULL
);
