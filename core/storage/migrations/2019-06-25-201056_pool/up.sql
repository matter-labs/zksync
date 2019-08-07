create table mempool
(
    hash                    bytea primary key,
    primary_account_address text     not null,
    nonce                   bigint    not null,
    tx                      jsonb     not null,
    created_at              timestamp not null default now()
);

create table executed_transactions
(
    id           serial primary key,
    block_number bigint not null,
    tx_hash      bytea not null,
    operation    jsonb,
    success     bool not null,
    fail_reason text
);
