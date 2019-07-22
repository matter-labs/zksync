create table mempool
(
    hash            bytea primary key,
    primary_account integer,
    nonce           bigint    not null,
    tx              jsonb     not null,
    created_at      timestamp not null default now()
);