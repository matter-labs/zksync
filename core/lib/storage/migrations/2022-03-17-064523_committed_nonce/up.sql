CREATE TABLE IF NOT EXISTS committed_nonce
(
    account_id    bigint not null primary key,
    nonce         bigint not null
);
