-- Your SQL goes here

-- commit blocks, execute blocks, proof blocks, verify blocks on ethereum
CREATE TABLE aggregate_operations (
    id bigserial PRIMARY KEY,
    action_type TEXT NOT NULL,
    arguments jsonb NOT NULL,
    from_block bigint not null,
    to_block bigint not null,
    created_at TIMESTAMP with time zone NOT NULL DEFAULT now()
);

CREATE TABLE eth_aggregated_ops_binding (
    id bigserial PRIMARY KEY,
    op_id bigserial NOT NULL REFERENCES aggregate_operations(id),
    eth_op_id bigserial NOT NULL REFERENCES eth_operations(id)
);

CREATE TABLE eth_unprocessed_aggregated_ops (
    op_id bigserial NOT NULL REFERENCES aggregate_operations(id),
    PRIMARY KEY (op_id)
);
