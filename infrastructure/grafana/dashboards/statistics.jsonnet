local G = import '../generator.libsonnet';

local stat(title, metric) =
  G.grafana.statPanel.new(
    title,
    datasource = 'Prometheus',
    reducerFunction = 'last'
  ).addTarget(
    G.grafana.prometheus.target(
      metric,
      legendFormat = '{{namespace}}'
    )
  ) + { gridPos: { h: G.height, w: G.width } };

G.dashboardRaw(
  'statistics',
  [
    stat('COMMIT not confirmed operations', 'count_operations{action="COMMIT", confirmed="false"}'),
    stat('VERIFY not confirmed operations', 'count_operations{action="VERIFY", confirmed="false"}'),
    stat('COMMIT confirmed operations', 'count_operations{action="COMMIT", confirmed="true"}'),
    stat('VERIFY confirmed operations', 'count_operations{action="VERIFY", confirmed="true"}'),
    stat('Transaction batch sizes', 'tx_batch_size'),
  ]
)

