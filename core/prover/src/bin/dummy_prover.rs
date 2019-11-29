use log::info;
use models::node::config::{PROVER_CYCLE_WAIT, PROVER_TIMEOUT};
use models::EncodedProof;
use std::time::Duration;
use storage::ConnectionPool;

fn main() {
    env_logger::init();

    let pool = ConnectionPool::new();
    let worker = "dummy_worker";
    info!("Started prover");
    loop {
        let storage = pool.access_storage().expect("Storage access");
        let job = storage
            .job_for_unverified_block(worker, PROVER_TIMEOUT)
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
