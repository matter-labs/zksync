local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.operations.add_complete_withdrawals_transaction",
  "sql.chain.operations.add_pending_withdrawal",
  "sql.chain.operations.eth_tx_for_withdrawal",
  "sql.chain.operations.get_executed_operation",
  "sql.chain.operations.get_executed_priority_operation",
  "sql.chain.operations.get_executed_priority_operation_by_hash",
  "sql.chain.operations.get_last_block_by_action",
  "sql.chain.operations.store_executed_priority_op",
  "sql.chain.operations.confirm_operation",
  "sql.chain.operations.get_operation",
  "sql.chain.operations.store_executed_tx",
  "sql.chain.operations.store_operation",
  "sql.chain.operations_ext.account_created_on",
  "sql.chain.operations_ext.find_priority_op_by_hash",
  "sql.chain.operations_ext.get_account_transactions_history",
  "sql.chain.operations_ext.get_account_transactions_history_from",
  "sql.chain.operations_ext.get_priority_op_receipt",
  "sql.chain.operations_ext.find_tx_by_hash",
  "sql.chain.operations_ext.tx_receipt",
];

G.dashboard(
  'Metrics / sql / chain / operations',
  [ G.panel(metric) for metric in metrics ]
)
