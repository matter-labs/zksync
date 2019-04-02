-- Your SQL goes here

CREATE TABLE proofs (
    block_number    serial primary key,
    created_at      timestamp not null default now(),
    started_at      timestamp not null default now(),
    finished_at     timestamp,
    proof           jsonb,
    worker          text
);
