local G = import '../generator.libsonnet';
local metrics = [
  "sql.ethereum.add_hash_entry",
  "sql.ethereum.confirm_eth_tx",
  "sql.ethereum.get_eth_op_id",
  "sql.ethereum.is_aggregated_op_confirmed",
  "sql.ethereum.get_next_nonce",
  "sql.ethereum.initialize_eth_data",
  "sql.ethereum.load_average_gas_price",
  "sql.ethereum.load_eth_params",
  "sql.ethereum.load_gas_price_limit",
  "sql.ethereum.load_stats",
  "sql.ethereum.load_unconfirmed_operations",
  "sql.ethereum.restore_unprocessed_operations",
  "sql.ethereum.load_unprocessed_operations",
  "sql.ethereum.report_created_operation",
  "sql.ethereum.save_new_eth_tx",
  "sql.ethereum.update_eth_tx",
  "sql.ethereum.update_gas_price",
];

G.dashboard('sql / ethereum', metrics)
