-- Your SQL goes here

ALTER TABLE blocks ADD commitment BYTEA NOT NULL;
ALTER TABLE pending_block ADD previous_root_hash BYTEA NOT NULL;
