local G = import '../generator.libsonnet';
local metrics = [
  "api.notifier.add_account_update_sub",
  "api.notifier.add_priority_op_sub",
  "api.notifier.add_transaction_sub",
  "api.notifier.handle_executed_operations",
  "api.notifier.handle_new_block",
];

G.dashboard(
  'Metrics / notifier',
  [ G.panel(metric) for metric in metrics ]
)
