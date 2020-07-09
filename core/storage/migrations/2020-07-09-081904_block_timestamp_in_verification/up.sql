CREATE TABLE eth_last_known_timestamp (
    unix_timestamp bigserial NOT NULL,
    PRIMARY KEY (unix_timestamp)
);
ALTER TABLE data_restore_rollup_ops ADD block_timestamp bigserial;
ALTER TABLE blocks ADD block_timestamp bigserial;
