-- Stored generated witness for block.
CREATE TABLE block_witness
(
    block BIGINT REFERENCES blocks (number) ON UPDATE CASCADE ON DELETE CASCADE,
    witness jsonb NOT NULL,
    PRIMARY KEY (block)
);
