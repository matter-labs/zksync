CREATE TABLE operations (
    id              serial primary key,
    block_number    integer not null,
    data            jsonb not null,
    addr            text not null,
    nonce           serial not null,
    created_at      timestamp not null default now()
);

-- TODO: CREATE INDEX op_type_index ON operations (data.type); 

CREATE TABLE accounts (
    id          integer not null primary key,
    data        json not null
);

CREATE TABLE account_updates (
    account_id      integer not null,
    block_number    integer not null,
    data            json not null,
    PRIMARY KEY (account_id, block_number)
);

CREATE INDEX account_updates_block_index ON account_updates (block_number);
