local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.state.apply_state_update",
  "sql.chain.state.commit_state_update",
  "sql.chain.state.load_committed_state",
  "sql.chain.state.load_state_diff",
  "sql.chain.state.load_state_diff",
  "sql.chain.state.load_verified_state",
  "sql.chain.state.remove_account_balance_updates",
  "sql.chain.state.remove_account_creates",
  "sql.chain.state.remove_account_pubkey_updates",
];

G.dashboard('sql / chain / state', metrics)
