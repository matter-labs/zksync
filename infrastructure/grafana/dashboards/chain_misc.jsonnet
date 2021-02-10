local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.account.account_address_by_id",
  "sql.chain.account.account_id_by_address",
  "sql.chain.account.account_state_by_address",
  "sql.chain.account.account_state_by_id",
  "sql.chain.account.get_account_and_last_block",
  "sql.chain.account.last_committed_state_for_account",
  "sql.chain.account.last_verified_state_for_account",
  "sql.chain.account.account_type_by_id",
  "sql.chain.account.set_account_type",
  "sql.chain.stats.count_outstanding_proofs",
  "sql.chain.stats.count_total_transactions",
];

G.dashboard('sql / chain / account & stats', metrics)
