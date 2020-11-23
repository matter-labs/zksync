local grafana = import 'grafonnet/grafana.libsonnet';
local height = 10;
local width = 1337;
local metrics = [
  'committer_save_pending_block',
  'committer_commit_block',
];

local panel(metric, span = '5m') = 
  grafana.graphPanel.new(
    title = metric,
    datasource = 'Prometheus',
  ).addTarget(
    grafana.prometheus.target(
      'rate(%s_sum[%s]) / rate(%s_count[%s])' % [metric, span, metric, span],
      legendFormat = '%s({{namespace}})' % metric
    )
  ) + { gridPos: { h: height, w: width } };


grafana.dashboard.new(
  'Metrics / committer',
  schemaVersion = 18,
  editable = true
).addPanels([
  panel(metrics[0]),
  panel(metrics[1], '1d')
])
