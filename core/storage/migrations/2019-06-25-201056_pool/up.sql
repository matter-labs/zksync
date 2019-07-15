create table mempool
(
    from_account integer   not null,
    nonce        bigint    not null,
    tx           jsonb     not null,
    created_at   timestamp not null default now(),
    PRIMARY KEY (from_account, nonce)
);