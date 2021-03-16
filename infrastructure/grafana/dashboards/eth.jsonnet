local G = import '../generator.libsonnet';
local metrics = [
  "eth_watcher.get_complete_withdrawals_event",
  "eth_watcher.get_priority_op_events_with_blocks",
  "eth_watcher.get_priority_op_events",
  "eth_watcher.get_new_tokens_event",
  "eth_watcher.poll_eth_node",
  "eth_sender.load_new_operations",
  "eth_sender.perform_commitment_step",
  "eth_sender.proceed_next_operations",
];

G.dashboard('eth_sender & eth_watcher', metrics)
