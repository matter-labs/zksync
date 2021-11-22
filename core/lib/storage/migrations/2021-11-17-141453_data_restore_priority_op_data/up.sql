CREATE TABLE data_restore_priority_op_data (
    serial_id BIGINT NOT NULL,
    op JSONB NOT NULL,
    PRIMARY KEY (serial_id)
);
