local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.mempool.collect_garbage",
  "sql.chain.mempool.insert_batch",
  "sql.chain.mempool.insert_tx",
  "sql.chain.mempool.load_txs",
  "sql.chain.mempool.remove_tx",
  "sql.chain.mempool.remove_txs",
  "sql.chain.mempool.return_executed_txs_to_mempool",
];

G.dashboard('sql / chain / mempool', metrics)
