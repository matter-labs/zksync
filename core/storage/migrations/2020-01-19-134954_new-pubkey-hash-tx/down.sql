-- This file should undo anything in `up.sql`
DROP TABLE IF EXISTS account_pubkey_updates CASCADE;

ALTER TABLE accounts
    DROP COLUMN pubkey_hash CASCADE;
