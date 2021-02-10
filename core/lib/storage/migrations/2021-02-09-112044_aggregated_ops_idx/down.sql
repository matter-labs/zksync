-- This file should undo anything in `up.sql`

drop index aggregate_ops_range_idx;
drop index eth_agg_op_binding_idx;

truncate commit_aggregated_blocks_binding;
truncate execute_aggregated_blocks_binding;

drop table commit_aggregated_blocks_binding;
drop table execute_aggregated_blocks_binding;
