ALTER TABLE account_tree_cache DROP COLUMN tree_cache_binary;
ALTER TABLE account_tree_cache ALTER COLUMN tree_cache SET NOT NULL;
