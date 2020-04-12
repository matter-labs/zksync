pub mod cli_utils;
pub mod client;
pub mod exit_proof;
pub mod plonk_step_by_step_prover;
pub mod prover_data;
pub mod serialization;

// Built-in deps
use std::sync::{mpsc, Arc};
use std::{fmt, thread};
// External deps
use log::*;
// Workspace deps
use models::node::config::PROVER_CYCLE_WAIT;
use models::node::Engine;
use models::prover_utils::EncodedProofPlonk;
use std::time::Duration;

pub trait ProverConfig {
    fn from_env() -> Self;
}

pub trait ProverImpl<C: ApiClient> {
    type Config: ProverConfig;
    fn create_from_config(config: Self::Config, client: C, heartbeat: Duration) -> Self;
    fn next_round(
        &self,
        start_heartbeats_tx: mpsc::Sender<(i32, bool)>,
    ) -> Result<(), BabyProverError>;
    fn get_heartbeat_options(&self) -> (&C, Duration);
}

pub trait ApiClient {
    fn block_to_prove(&self, block_size: usize) -> Result<Option<(i64, i32)>, failure::Error>;
    fn working_on(&self, job_id: i32) -> Result<(), failure::Error>;
    fn prover_data(
        &self,
        block: i64,
    ) -> Result<circuit::circuit::FranklinCircuit<'_, Engine>, failure::Error>;
    fn publish(&self, block: i64, p: EncodedProofPlonk) -> Result<(), failure::Error>;
}

#[derive(Debug)]
pub enum BabyProverError {
    Api(String),
    Internal(String),
    Stop,
}

impl fmt::Display for BabyProverError {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let desc = match self {
            BabyProverError::Api(s) => s,
            BabyProverError::Internal(s) => s,
            BabyProverError::Stop => "stop",
        };
        write!(f, "{}", desc)
    }
}

pub fn start<C: 'static + Sync + Send + ApiClient, P: ProverImpl<C> + Send + Sync + 'static>(
    prover: P,
    exit_err_tx: mpsc::Sender<BabyProverError>,
) {
    let (tx_block_start, rx_block_start) = mpsc::channel();
    let prover = Arc::new(prover);
    let prover_rc = Arc::clone(&prover);
    let join_handle = thread::spawn(move || {
        let tx_block_start2 = tx_block_start.clone();
        exit_err_tx
            .send(run_rounds(prover.as_ref(), tx_block_start))
            .expect("failed to send exit error");
        tx_block_start2
            .send((0, true))
            .expect("failed to send heartbeat exit request"); // exit heartbeat routine request.
    });
    keep_sending_work_heartbeats(prover_rc.get_heartbeat_options(), rx_block_start);
    join_handle
        .join()
        .expect("failed to join on running rounds thread");
}

fn run_rounds<P: ProverImpl<C>, C: ApiClient>(
    p: &P,
    start_heartbeats_tx: mpsc::Sender<(i32, bool)>,
) -> BabyProverError {
    info!("Running worker rounds");

    loop {
        trace!("Starting a next round");
        let ret = p.next_round(start_heartbeats_tx.clone());
        if let Err(err) = ret {
            match err {
                BabyProverError::Api(text) => {
                    error!("could not reach api server: {}", text);
                }
                BabyProverError::Internal(_) => {
                    return err;
                }
                _ => {}
            };
        }
        trace!("round completed.");
        thread::sleep(PROVER_CYCLE_WAIT);
    }
}

fn keep_sending_work_heartbeats<C: ApiClient>(
    heartbeat_opts: (&C, Duration),
    start_heartbeats_rx: mpsc::Receiver<(i32, bool)>,
) {
    let mut job_id = 0;
    loop {
        thread::sleep(heartbeat_opts.1);
        let (j, quit) = match start_heartbeats_rx.try_recv() {
            Ok(v) => v,
            Err(mpsc::TryRecvError::Empty) => (job_id, false),
            Err(e) => {
                panic!("error receiving from hearbeat channel: {}", e);
            }
        };
        if quit {
            return;
        }
        job_id = j;
        if job_id != 0 {
            trace!("sending working_on request for job_id: {}", job_id);
            let ret = heartbeat_opts.0.working_on(job_id);
            if let Err(e) = ret {
                error!("working_on request errored: {}", e);
            }
        }
    }
}
