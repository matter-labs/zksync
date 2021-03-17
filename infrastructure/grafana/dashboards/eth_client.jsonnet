local G = import '../generator.libsonnet';
local metrics = [
  "eth_client.direct.pending_nonce",
  "eth_client.direct.current_nonce",
  "eth_client.direct.current_nonce",
  "eth_client.direct.get_gas_price",
  "eth_client.direct.block",
  "eth_client.direct.balance",
  "eth_client.direct.sign_prepared_tx_for_addr",
  "eth_client.direct.send_raw_tx",
  "eth_client.direct.tx_receipt",
  "eth_client.direct.failure_reason",
  "eth_client.direct.eth_balance",
  "eth_client.direct.allowance",
  "eth_client.direct.call_contract_function",
  "eth_client.direct.get_tx_status",
  "eth_client.direct.logs",
];

G.dashboard('eth_client', metrics)
