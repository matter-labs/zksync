local G = import '../generator.libsonnet';
local metrics = [
  "sql.token.get_count",
  "sql.token.get_historical_ticker_price",
  "sql.token.get_token",
  "sql.token.load_tokens",
  "sql.token.load_tokens_by_market_volume",
  "sql.token.store_token",
  "sql.token.update_historical_ticker_price",
  "sql.token.update_market_volume",
];

G.dashboard('sql / token', metrics)
