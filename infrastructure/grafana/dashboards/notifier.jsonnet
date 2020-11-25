local G = import '../generator.libsonnet';
local metrics = [
  "api.notifier.add_account_update_sub",
  "api.notifier.add_priority_op_sub",
  "api.notifier.add_transaction_sub",
  "api.notifier.handle_executed_operations",
  "api.notifier.handle_new_block",
  "api.notifier.get_tx_receipt",
  "api.notifier.get_block_info",
  "api.notifier.get_executed_priority_operation",
  "api.notifier.get_account_info",
  "api.notifier.get_account_state",
];

G.dashboard(
  'Metrics / notifier',
  [ G.panel(metric) for metric in metrics ]
)
