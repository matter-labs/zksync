CREATE TABLE prover_job_queue (
    id serial primary key,
    job_status int not null,
    job_priority int not null,
    job_type text not null,

    created_at timestamp with time zone not null default now(),
    updated_by text not null,
    updated_at timestamp with time zone not null default now(),

    first_block bigint not null,
    last_block bigint not null,
    job_data jsonb not null
);

CREATE TABLE aggregated_proofs (
    first_block bigint not null,
    last_block bigint not null,
    created_at timestamp with time zone not null default now(),
    proof jsonb not null,
    PRIMARY KEY (first_block, last_block)
)
