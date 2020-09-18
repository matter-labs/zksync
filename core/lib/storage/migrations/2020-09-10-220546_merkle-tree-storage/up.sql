-- Stored cache for account merkle tree.
CREATE TABLE account_tree_cache
(
    block BIGINT REFERENCES blocks (number) ON UPDATE CASCADE ON DELETE CASCADE,
    tree_cache jsonb NOT NULL,
    PRIMARY KEY (block)
);
