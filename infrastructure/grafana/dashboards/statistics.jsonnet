local G = import '../generator.libsonnet';

local stat(title, metric, x=0, y=0) =
  G.grafana.statPanel.new(
    title,
    datasource = 'Prometheus',
    reducerFunction = 'last'
  ).addTarget(
    G.grafana.prometheus.target(
      metric,
      legendFormat = '{{namespace}}'
    )
  ) + { gridPos: { h: G.height, w: 12, 'x': x, 'y': y } };

local pie(title, metrics) =
  local addSlice(chart, metric) =
    local formatted = std.strReplace(metric, '.', '_');
    chart.addTarget(
      G.grafana.prometheus.target(
        'avg(rate(%s_sum[1d]))' % [formatted],
        legendFormat = metric,
      )
    );
  local chart = G.grafana.pieChartPanel.new(title);
  std.foldl(addSlice, metrics, chart);

local metrics = [
    "eth_sender.load_new_operations",
    "sql.chain.stats.count_total_transactions",
    "sql.chain.block.get_block_executed_ops",
    "sql.chain.block.get_block",
    "api.rpc.account_info",
    "sql.chain.account.account_state_by_id",
    "sql.chain.account.last_committed_state_for_account",
    "sql.chain.operations_ext.get_account_transactions_history",
    "sql.chain.account.account_state_by_address",
    "api.rpc.get_account_state",
    "sql.connection_acquire",
    "sql.ethereum.load_unprocessed_operations",
    "ticker.get_tx_fee",
    "api.rpc.get_tx_fee",
    "ticker.get_last_quote",
    "api.rpc.get_token_price",
    "ticker.coingecko_request",
    "sql.chain.account.get_account_and_last_block",
    "eth_sender.proceed_next_operations",
    "eth_client.direct.tx_receipt",
    "eth_client.direct.get_tx_status",
    "eth_client.direct.current_nonce",
    "eth_client.direct.current_nonce",
    "witness_generator.prepare_witness_and_save_it",
    "ticker.get_token_price",
    "sql.chain.operations.get_last_block_by_aggregated_action",
];

G.dashboardRaw(
  'statistics',
  [
    pie('Time', metrics) + { gridPos: { h: G.height, w: 12, x: 0, y: 0 } },
    stat('Transaction batch sizes', 'tx_batch_size', 12, 0),
    stat('COMMIT not confirmed operations', 'count_operations{action="COMMIT", confirmed="false"}', 0, 10),
    stat('VERIFY not confirmed operations', 'count_operations{action="VERIFY", confirmed="false"}', 12, 10),
    stat('COMMIT confirmed operations', 'count_operations{action="COMMIT", confirmed="true"}', 0, 20),
    stat('VERIFY confirmed operations', 'count_operations{action="VERIFY", confirmed="true"}', 12, 20),
  ]
)

