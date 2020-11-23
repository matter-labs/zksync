local G = import '../generator.libsonnet';
local metrics = [
  'committer.commit_block',
  'committer.save_pending_block',
  'witness_generator.prepare_witness_and_save_it',
  'witness_generator.load_account_tree',
  'root_hash',
  'mempool.propose_new_block',
  'signature_checker.verify_eth_signature_single_tx',
  'signature_checker.verify_eth_signature_txs_batch',
];

G.dashboard(
  'Metrics / miscellaneous',
  [ G.panel(metric) for metric in metrics ]
)
