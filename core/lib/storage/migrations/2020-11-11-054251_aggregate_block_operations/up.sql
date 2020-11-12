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


