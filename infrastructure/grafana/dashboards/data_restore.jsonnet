local G = import '../generator.libsonnet';
local metrics = [
  "sql.data_restore.load_last_watched_block_number",
  "sql.data_restore.update_last_watched_block_number",
  "sql.data_restore.initialize_eth_stats",
  "sql.data_restore.load_events_state",
  "sql.data_restore.load_rollup_ops_blocks",
  "sql.data_restore.load_storage_state",
  "sql.data_restore.save_rollup_ops",
  "sql.data_restore.update_block_events",
  "sql.data_restore.update_storage_state",
];

G.dashboard(
  'Metrics / data_restore',
  [ G.panel(metric) for metric in metrics ]
)
