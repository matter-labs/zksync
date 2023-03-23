CREATE table account_tree_cache_new
(
    block BIGINT REFERENCES blocks (number) ON DELETE CASCADE,
    tree_cache_binary BYTEA,
    PRIMARY KEY (block)
);
