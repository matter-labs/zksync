create TABLE account_pubkey_updates
(
    pubkey_update_id serial  not null,
    account_id        bigint not null,
    block_number      bigint not null,
    old_pubkey_hash   bytea not null,
    new_pubkey_hash   bytea not null,
    old_nonce         bigint  not null,
    new_nonce         bigint  not null,
    PRIMARY KEY (pubkey_update_id)
);

ALTER TABLE accounts
    ADD COLUMN pubkey_hash bytea not null;
