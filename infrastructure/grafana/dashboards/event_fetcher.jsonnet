local G = import '../generator.libsonnet';
local metrics = [
  "api.event_fetcher.last_committed_block",
  "api.event_fetcher.last_verified_block",
  "api.event_fetcher.load_operation",
  "api.event_fetcher.load_pending_block",
  "api.event_fetcher.send_operations",
  "api.event_fetcher.update_pending_block",
];

G.dashboard(
  'Metrics / event_fetcher',
  [ G.panel(metric) for metric in metrics ]
)
