{
  grafana:: import 'grafonnet-lib/grafonnet/grafana.libsonnet',

  panel(metric, span = '1h')::
    local width = 1337;
    local height = 10;
    local formatted = std.strReplace(metric, '.', '_');
    $.grafana.graphPanel.new(
      title = metric,
      datasource = 'Prometheus',
    ).addTarget(
      $.grafana.prometheus.target(
        'rate(%s_sum[%s]) / rate(%s_count[%s])' 
          % [formatted, span, formatted, span],
        legendFormat = '{{namespace}}'
      )
    ) + { gridPos: { h: height, w: width } },

  dashboard(title = '', panels = [])::
    $.grafana.dashboard.new(
      title,
      schemaVersion = 18,
      editable = true,
      refresh = '1m'
    ).addPanels(panels)
}
