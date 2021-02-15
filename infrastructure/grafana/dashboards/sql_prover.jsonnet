local G = import '../generator.libsonnet';
local metrics = [
  "sql.prover.get_existing_prover_run",
  "sql.prover.get_witness",
  "sql.prover.load_proof",
  "sql.prover.pending_jobs_count",
  "sql.prover.prover_run_for_next_commit",
  "sql.prover.record_prover_is_working",
  "sql.prover.record_prover_stop",
  "sql.prover.register_prover",
  "sql.prover.store_proof",
  "sql.prover.store_witness",
  "sql.prover.unstarted_jobs_count",
];

G.dashboard('sql / prover', metrics)
