local G = import '../generator.libsonnet';
local metrics = [
  "sql.ethereum.add_hash_entry",
  "sql.ethereum.confirm_eth_tx",
  "sql.ethereum.get_eth_op_id",
  "sql.ethereum.get_next_nonce",
  "sql.ethereum.initialize_eth_data",
  "sql.ethereum.load_unconfirmed_operations",
  "sql.ethereum.load_unprocessed_operations",
  "sql.ethereum.report_created_operation",
  "sql.ethereum.save_new_eth_tx",
  "sql.ethereum.update_eth_tx",
  "sql.ethereum.update_gas_price",
];

G.dashboard(
  'Metrics / sql / ethereum',
  [ G.panel(metric) for metric in metrics ]
)
