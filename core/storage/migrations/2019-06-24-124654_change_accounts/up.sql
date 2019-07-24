ALTER TABLE accounts
    DROP COLUMN data CASCADE,
    ADD COLUMN nonce BIGINT not null,
    ADD COLUMN address bytea not null;

CREATE TABLE tokens
(
    id      integer not null primary key,
    address text    not null
);

-- Add ETH token
INSERT INTO tokens
values (0, '');

CREATE TABLE balances
(
    account_id integer REFERENCES accounts (id) ON UPDATE CASCADE ON DELETE CASCADE,
    coin_id    integer REFERENCES tokens (id) ON UPDATE CASCADE,
    balance    numeric not null default 0,
    PRIMARY KEY (account_id, coin_id)
);


DROP TABLE account_updates cascade;

create TABLE account_balance_updates
(
    balance_update_id serial  not null,
    account_id        integer not null,
    block_number      integer not null,
    coin_id           integer not null references tokens (id) on update cascade,
    old_balance       numeric not null,
    new_balance       numeric not null,
    old_nonce         bigint  not null,
    new_nonce         bigint  not null,
    PRIMARY KEY (balance_update_id)
);

CREATE TABLE account_creates
(
    account_id   integer not null,
    is_create    bool    not null,
    block_number integer not null,
    address      bytea   not null,
    nonce        bigint  not null,
    PRIMARY KEY (account_id, block_number)
);


ALTER TABLE transactions
    drop column nonce cascade,
    add column nonce integer not null,
    add column token integer not null;
