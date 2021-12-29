DROP TABLE IF EXISTS incomplete_blocks;
ALTER TABLE pending_block
    ADD timestamp bigint,
    ADD previous_root_hash BYTEA NOT NULL default '\x0000000000000000000000000000000000000000000000000000000000000000';

ALTER TABLE block_metadata ADD CONSTRAINT block_metadata_block_number_fkey
    FOREIGN KEY (block_number) REFERENCES blocks(number) ON DELETE CASCADE;
