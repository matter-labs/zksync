local G = import '../generator.libsonnet';
local metrics = [
  "api.v01.block_by_id",
  "api.v01.block_transactions",
  "api.v01.block_tx",
  "api.v01.blocks",
  "api.v01.executed_tx_by_hash",
  "api.v01.explorer_search",
  "api.v01.priority_op",
  "api.v01.status",
  "api.v01.testnet_config",
  "api.v01.tokens",
  "api.v01.tx_by_hash",
  "api.v01.tx_history",
  "api.v01.tx_history_newer_than",
  "api.v01.tx_history_older_than",
  "api.v01.withdrawal_processing_time",
];

G.dashboard(
  'Metrics / api v0.1',
  [ G.panel(metric) for metric in metrics ]
)
