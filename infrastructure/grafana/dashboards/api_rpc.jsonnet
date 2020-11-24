local G = import '../generator.libsonnet';
local metrics = [
  "api.rpc.get_confirmations_for_eth_op_amount",
  "api.rpc.account_info",
  "api.rpc.contract_address",
  "api.rpc.ethop_info",
  "api.rpc.get_eth_tx_for_withdrawal",
  "api.rpc.get_token_price",
  "api.rpc.get_tx_fee",
  "api.rpc.get_txs_batch_fee_in_wei",
  "api.rpc.submit_txs_batch",
  "api.rpc.tokens",
  "api.rpc.tx_info",
  "api.rpc.tx_submit",
];

G.dashboard(
  'Metrics / rpc',
  [ G.panel(metric) for metric in metrics ]
)
