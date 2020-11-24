local G = import '../generator.libsonnet';
local metrics = [
  "sql.token.get_count",
  "sql.token.get_historical_ticker_price",
  "sql.token.get_token",
  "sql.token.load_tokens",
  "sql.token.store_token",
  "sql.token.update_historical_ticker_price",
];

G.dashboard(
  'Metrics / sql / token',
  [ G.panel(metric) for metric in metrics ]
)
