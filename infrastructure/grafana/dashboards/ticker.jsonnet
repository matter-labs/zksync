local G = import '../generator.libsonnet';
local metrics = [
  "ticker.get_gas_price_wei",
  "ticker.get_historical_ticker_price",
  "ticker.get_last_quote",
  "ticker.get_token",
  "ticker.dispatcher.request",
  "ticker.get_tx_fee",
  "ticker.get_token_price",
  "ticker.is_token_allowed",
  "ticker.coingecko_request",
  "ticker.validator.update_all_tokens",
  "ticker.validator.check_token",
  "ticker.uniswap_watcher.get_market_volume"
];

G.dashboard(
  'Metrics / ticker',
  [ G.panel(metric) for metric in metrics ]
)
