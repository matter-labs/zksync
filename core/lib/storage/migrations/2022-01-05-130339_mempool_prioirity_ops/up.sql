CREATE TABLE mempool_priority_operations (
    serial_id BIGINT primary key,
    data JSONB NOT NULL,
    l1_address BYTEA NOT NULL,
    l2_address BYTEA NOT NULL,
    type TEXT NOT NULL,
    deadline_block BIGINT NOT NULL,
    eth_hash BYTEA NOT NULL,
    eth_block BIGINT NOT NULL,
    eth_block_index INTEGER NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE NOT NULL
);