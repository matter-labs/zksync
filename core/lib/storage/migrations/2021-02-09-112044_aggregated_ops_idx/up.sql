CREATE TABLE execute_aggregated_blocks_binding
(
    op_id        bigserial NOT NULL REFERENCES aggregate_operations (id) on delete cascade,
    block_number bigserial NOT NULL REFERENCES blocks (number) on delete cascade,
    primary key (block_number)
);

CREATE TABLE commit_aggregated_blocks_binding
(
    op_id        bigserial NOT NULL REFERENCES aggregate_operations (id) on delete cascade,
    block_number bigserial NOT NULL REFERENCES blocks (number) on delete cascade,
    primary key (block_number)
);


INSERT INTO execute_aggregated_blocks_binding (op_id, block_number)
SELECT aggregate_operations.id, blocks.number
from aggregate_operations
         INNER JOIN blocks ON blocks.number BETWEEN aggregate_operations.from_block AND aggregate_operations.to_block
WHERE aggregate_operations.action_type = 'ExecuteBlocks';

INSERT INTO commit_aggregated_blocks_binding (op_id, block_number)
SELECT aggregate_operations.id, blocks.number
from aggregate_operations
         INNER JOIN blocks ON blocks.number BETWEEN aggregate_operations.from_block AND aggregate_operations.to_block
WHERE aggregate_operations.action_type = 'CommitBlocks';

create index eth_agg_op_binding_idx on eth_aggregated_ops_binding using BTREE (op_id);
create index aggregate_ops_range_idx on aggregate_operations using BTREE (from_block, to_block desc);
