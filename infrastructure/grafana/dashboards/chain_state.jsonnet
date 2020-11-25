local G = import '../generator.libsonnet';
local metrics = [
  "sql.chain.state.apply_state_update",
  "sql.chain.state.commit_state_update",
  "sql.chain.state.load_committed_state",
  "sql.chain.state.load_state_diff",
  "sql.chain.state.load_state_diff",
  "sql.chain.state.load_verified_state",
];

G.dashboard(
  'Metrics / sql / chain / state',
  [ G.panel(metric) for metric in metrics ]
)
