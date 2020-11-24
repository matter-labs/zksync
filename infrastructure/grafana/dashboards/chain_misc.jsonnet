local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.account.account_state_by_address",
  "sql.chain.account.get_account_and_last_block",
  "sql.chain.account.last_committed_state_for_account",
  "sql.chain.account.last_verified_state_for_account",
  "sql.chain.stats.count_outstanding_proofs",
  "sql.chain.stats.count_total_transactions",
];

G.dashboard(
  'Metrics / sql / chain / account & stats',
  [ G.panel(metric) for metric in metrics ]
)
