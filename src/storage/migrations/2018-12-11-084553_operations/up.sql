-- op_config table is used to keep track of nonce sequences for different sender addresses
CREATE TABLE op_config(
    addr        text primary key,   -- sender address for ETH
    next_nonce  integer             -- nonce sequence holder
);
INSERT INTO op_config VALUES ('0x0', 0);
CREATE RULE noins_op_config AS ON INSERT TO op_config DO NOTHING;
CREATE RULE nodel_op_config AS ON DELETE TO op_config DO NOTHING;

CREATE OR REPLACE FUNCTION op_config_next_nonce() RETURNS integer AS
$$
BEGIN
    UPDATE op_config SET next_nonce = next_nonce + 1;
    RETURN (SELECT next_nonce - 1 from op_config);
END;
$$ LANGUAGE 'plpgsql';

CREATE OR REPLACE FUNCTION op_config_addr() RETURNS text AS
$$
BEGIN
    RETURN (SELECT addr from op_config);
END;
$$ LANGUAGE 'plpgsql';

CREATE TABLE operations (
    id              serial primary key,
    data            jsonb not null,
    addr            text not null default op_config_addr(),
    nonce           integer not null default op_config_next_nonce(),
    block_number    integer not null,
    action_type     text not null,
    created_at      timestamp not null default now()
);

CREATE INDEX operations_block_index ON operations (block_number);

CREATE TABLE accounts (
    id              integer not null primary key,
    last_block      integer not null,
    data            json not null
);

CREATE INDEX accounts_block_index ON accounts (last_block);

CREATE TABLE account_updates (
    account_id      integer not null,
    block_number    integer not null,
    data            json not null,
    PRIMARY KEY (account_id, block_number)
);

CREATE INDEX account_updates_block_index ON account_updates (block_number);

