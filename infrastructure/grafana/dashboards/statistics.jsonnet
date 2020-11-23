local G = import '../generator.libsonnet';

local gauge(title, metric) =
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

G.dashboard(
  'Metrics / statistics',
  [
    gauge('COMMIT not confirmed operations', 'count_operations{action="COMMIT", confirmed="false"}'),
    gauge('VERIFY not confirmed operations', 'count_operations{action="VERIFY", confirmed="false"}'),
    gauge('COMMIT confirmed operations', 'count_operations{action="COMMIT", confirmed="true"}'),
    gauge('VERIFY confirmed operations', 'count_operations{action="VERIFY", confirmed="true"}'),
    gauge('Transaction batch sizes', 'tx_batch_size'),
  ]
)

