CREATE TABLE blocks (
    block_number    serial primary key,   -- block number
    block_data      json not null
);

CREATE TABLE accounts (
    id                  serial not null primary key,       -- account id
    last_block_number   integer,                           -- last updated in block
    nonce               integer not null,
    balance             numeric(80) not null,              -- amount of the tx
    pub_x               numeric(80),                       -- for registrations only: pub key
    pub_y               numeric(80)                        -- for registrations only: pub key
);

CREATE INDEX account_block_index ON accounts (last_block_number);
