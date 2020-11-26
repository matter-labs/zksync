local G = import '../generator.libsonnet';
local metrics = [
  "state.change_pubkey",
  "state.deposit",
  "state.forced_exit",
  "state.full_exit",
  "state.transfer",
  "state.transfer_to_new",
  "state.transfer_to_self",
  "state.withdraw",
];

G.dashboard(
  'Metrics / plasma (state)',
  [ G.panel(metric) for metric in metrics ]
)
