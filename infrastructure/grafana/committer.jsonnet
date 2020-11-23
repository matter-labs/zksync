local G = import './generator.libsonnet';
local metrics = [
  'committer.save_pending_block',
  'committer.commit_block',
];

G.dashboard(
  'Metrics / committer', 
  [
    G.panel(metrics[0]),
    G.panel(metrics[1], '4h')
  ]
)
