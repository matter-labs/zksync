{
  grafana:: import 'grafonnet-lib/grafonnet/grafana.libsonnet',
  width:: 1337,
  height:: 10,

  time(metric, span = '1h')::
    local formatted = std.strReplace(metric, '.', '_');
    $.grafana.graphPanel.new(
      title = metric,
      datasource = 'Prometheus',
      format = 'ns',
    ).addTarget(
      $.grafana.prometheus.target(
        'rate(%s_sum[%s]) / rate(%s_count[%s])' 
          % [formatted, span, formatted, span],
        legendFormat = '{{namespace}}'
      )
    ) + { gridPos: { h: $.height, w: $.width } },

  samples(metric, span = '1h')::
    local formatted = std.strReplace(metric, '.', '_');
    $.grafana.graphPanel.new(
      title = metric + '[count]',
      datasource = 'Prometheus',
    ).addTarget(
      $.grafana.prometheus.target(
        'rate(%s_count[%s])' % [formatted, span],
        legendFormat = '{{namespace}}'
      )
    ) + { gridPos: { h: $.height, w: $.width } },

  dashboard(title, metrics = [])::
    $.grafana.dashboard.new(
      title,
      schemaVersion = 18,
      editable = true,
      refresh = '1m',
      tags = ['prometheus']
    ).addPanels(
      std.flattenArrays([
        [$.time(metric), $.samples(metric)]
        for metric in metrics
      ])
    ),

  dashboardRaw(title, panels = [])::
    $.grafana.dashboard.new(
      title,
      schemaVersion = 18,
      editable = true,
      refresh = '1m',
      tags = ['prometheus']
    ).addPanels(panels)
}
