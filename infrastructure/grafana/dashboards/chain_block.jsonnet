local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.block.find_block_by_height_or_hash",
  "sql.chain.block.get_last_verified_confirmed_block",
  "sql.chain.block.load_storage_pending_block",
  "sql.chain.block.execute_operation",
  "sql.chain.block.get_block",
  "sql.chain.block.get_block_executed_ops",
  "sql.chain.block.get_block_operations",
  "sql.chain.block.get_block_transactions",
  "sql.chain.block.get_last_committed_block",
  "sql.chain.block.get_last_verified_block",
  "sql.chain.block.get_storage_block",
  "sql.chain.block.load_block_range",
  "sql.chain.block.load_commit_op",
  "sql.chain.block.load_pending_block",
  "sql.chain.block.load_pending_block",
  "sql.chain.block.save_block",
  "sql.chain.block.save_block_transactions",
  "sql.chain.block.store_account_tree_cache",
];

G.dashboard(
  'Metrics / sql / chain / block',
  [ G.panel(metric) for metric in metrics ]
)
