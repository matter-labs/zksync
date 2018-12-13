CREATE TYPE op_type AS ENUM ('deposit', 'transfer', 'withdrawal');

CREATE TYPE tx AS (
    created_at          timestamp,
    account_id          integer,        -- account of the tx sender
    dst_id              integer,        -- for updates only: destination = tx.to
    amount              numeric(80),    -- amount of the tx
    pub_x               numeric(80),    -- for registrations only: pub key
    nonce               bigint,
    valid_until_block   integer,
    sig_r               numeric(80),
    sig_s               numeric(80)
);

CREATE TABLE block (
    block_number  serial primary key,   -- block number
    tx_type       op_type not null,     -- type of block
    created_at    timestamp not null,   -- block creation time
    root_hash     numeric(80),          -- root hash of the block
    transactions  tx[32] not null       -- list of transactions in the block
);

CREATE TABLE account (
    id                  serial primary key,       -- account id
    last_block_number   integer default 0,        -- last updated in block
    nonce               bigint default 0,
    amount              numeric(80) default 0,    -- amount of the tx
    pub_x               numeric(80) default 0     -- for registrations only: pub key
);

CREATE INDEX account_block_index ON account (last_block_number);
