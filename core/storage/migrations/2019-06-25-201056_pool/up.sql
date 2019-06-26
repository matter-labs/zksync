create table mempool (
    id serial primary key,
    tx jsonb not null,
    created_at timestamp not null default now()
);