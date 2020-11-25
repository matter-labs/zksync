local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.mempool.collect_garbage",
  "sql.chain.mempool.insert_batch",
  "sql.chain.mempool.insert_tx",
  "sql.chain.mempool.load_txs",
  "sql.chain.mempool.remove_tx",
  "sql.chain.mempool.remove_txs",
];

G.dashboard(
  'Metrics / sql / chain / mempool',
  [ G.panel(metric) for metric in metrics ]
)
