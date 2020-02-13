-- This file should undo anything in `up.sql`
DROP TABLE IF EXISTS account_pubkey_updates CASCADE;

ALTER TABLE accounts
    DROP COLUMN pubkey_hash CASCADE;

ALTER TABLE account_balance_updates
    DROP COLUMN update_order_id CASCADE;

ALTER TABLE account_creates
    DROP COLUMN update_order_id CASCADE;
