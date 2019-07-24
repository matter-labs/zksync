create table mempool
(
    hash            bytea primary key,
    primary_account_address bytea not null,
    nonce           bigint    not null,
    tx              jsonb     not null,
    created_at      timestamp not null default now()
);