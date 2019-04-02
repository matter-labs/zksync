-- Your SQL goes here

CREATE TABLE proofs (
    block_number    serial primary key,
    proof           jsonb not null,
    created_at      timestamp not null default now()
);
