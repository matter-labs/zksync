-- Your SQL goes here

CREATE TABLE proofs (
    block_number    bigserial primary key,
    proof           jsonb not null,
    created_at      timestamp not null default now()
);

CREATE TABLE prover_runs (
    id              serial primary key,
    block_number    bigint not null,
    worker          text,
    created_at      timestamp not null default now(),
    updated_at      timestamp not null default now()
);