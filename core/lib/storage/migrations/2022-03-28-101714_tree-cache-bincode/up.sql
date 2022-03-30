-- Adds an optional column for the tree cache encoded in binary.
-- Makes the old tree cache field optional.
ALTER TABLE account_tree_cache ALTER COLUMN tree_cache DROP NOT NULL;
ALTER TABLE account_tree_cache ADD COLUMN tree_cache_binary BYTEA;
