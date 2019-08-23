CREATE TABLE transactions (
    id              serial primary key,
  
    tx_type         text not null,
    from_account    bigint not null,
    to_account      bigint,
    nonce           bigint,
    amount          integer not null,
    fee             integer not null,

    block_number    bigint,
    state_root      text,

    created_at      timestamp not null default now()
);

CREATE INDEX transactions_block_index ON transactions (block_number);
