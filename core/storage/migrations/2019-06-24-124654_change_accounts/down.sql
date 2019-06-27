-- TODO (Drogan) update this file.

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
