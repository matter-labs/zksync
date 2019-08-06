ALTER TABLE accounts
    ADD COLUMN data json,
    DROP COLUMN nonce CASCADE,
    DROP COLUMN pk_x CASCADE,
    DROP COLUMN pk_y CASCADE;

DROP TABLE tokens cascade;

drop table balances cascade;

ALTER TABLE account_updates
    add column data json,
    drop column coin_id cascade,
    drop column balance cascade;

DROP TABLE account_creates cascade;

CREATE TABLE account_updates
(
    account_id   integer not null,
    block_number integer not null,
    data         json    not null,
    PRIMARY KEY (account_id, block_number)
);

CREATE INDEX account_updates_block_index ON account_updates (block_number);

drop table account_balance_updates cascade;
drop table account_creates cascade;
