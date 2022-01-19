DROP TABLE IF EXISTS incomplete_blocks;

ALTER TABLE block_metadata ADD CONSTRAINT block_metadata_block_number_fkey
    FOREIGN KEY (block_number) REFERENCES blocks(number) ON DELETE CASCADE;
