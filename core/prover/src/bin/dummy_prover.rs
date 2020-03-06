// Built-in deps
use std::time;
use std::time::Duration;
// External deps
use log::info;
// Workspace deps
use models::node::config::PROVER_CYCLE_WAIT;
use models::EncodedProof;
use storage::ConnectionPool;

fn main() {
    env_logger::init();

    let pool = ConnectionPool::new();
    let worker = "dummy_worker";
    info!("Started prover");
    for &block_size in models::params::block_chunk_sizes().iter().cycle() {
        let storage = pool.access_storage().expect("Storage access");
        let job = storage
            .prover_run_for_next_commit(worker, time::Duration::from_secs(10), block_size)
            .expect("prover job, db access");
        if let Some(job) = job {
            info!("Received job for block: {}", job.block_number);
            storage
                .store_proof(job.block_number as u32, &EncodedProof::default())
                .expect("db error");
        }
        std::thread::sleep(Duration::from_secs(PROVER_CYCLE_WAIT));
    }
}
