local G = import '../generator.libsonnet';
local metrics = [
  "state_keeper.apply_batch",
  "state_keeper.apply_priority_op",
  "state_keeper.apply_tx",
  // "state_keeper.create_genesis_block",
  "state_keeper.execute_proposed_block",
  // "state_keeper.initialize",
  "state_keeper.seal_pending_block",
  "state_keeper.store_pending_block",
];

G.dashboard(
  'Metrics / state_keeper',
  [ G.panel(metric) for metric in metrics ]
)
