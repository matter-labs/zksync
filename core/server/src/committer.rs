// Built-in uses
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
// External uses
use futures::channel::mpsc as fmpsc;
// Workspace uses
use models::config_options::ThreadPanicNotify;
use models::{Action, CommitRequest, Operation};
use storage::ConnectionPool;

pub fn start_committer(
    rx_for_ops: Receiver<CommitRequest>,
    tx_for_eth: Sender<Operation>,
    op_notify_sender: fmpsc::Sender<Operation>,
    pool: ConnectionPool,
    panic_notify: Sender<bool>,
) {
    thread::Builder::new()
        .name("committer".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            run_committer(rx_for_ops, tx_for_eth, op_notify_sender, pool);
        })
        .expect("thread creation failed");
}

fn run_committer(
    rx_for_ops: Receiver<CommitRequest>,
    tx_for_eth: Sender<Operation>,
    mut op_notify_sender: fmpsc::Sender<Operation>,
    pool: ConnectionPool,
) {
    info!("committer started");
    let storage = pool
        .access_storage()
        .expect("db connection failed for committer");

    let mut last_verified_block = storage.get_last_verified_block().expect("db failed");
    loop {
        let req = rx_for_ops.recv_timeout(Duration::from_millis(100));
        if let Ok(CommitRequest {
            block,
            accounts_updated,
        }) = req
        {
            if accounts_updated.is_empty() && block.number_of_processed_prior_ops() == 0 {
                info!(
                    "Failed transactions commited block: #{}",
                    block.block_number
                );
                storage
                    .save_block_transactions(&block)
                    .expect("commiter failed tx save");
                continue;
            }

            let op = Operation {
                action: Action::Commit,
                block,
                accounts_updated,
                id: None,
            };
            info!("commit block #{}", op.block.block_number);
            let op = storage
                .execute_operation(&op)
                .expect("committer must commit the op into db");

            tx_for_eth
                .send(op.clone())
                .expect("must send an operation for commitment to ethereum");

            // we notify about commit operation as soon as it is executed, we don't wait for eth confirmations
            op_notify_sender
                .try_send(op)
                .map_err(|e| warn!("Failed notify about commit op confirmation: {}", e))
                .unwrap_or_default();
            continue;
        } else {
            // there was a timeout, so check for the new ready proofs
            loop {
                let block_number = last_verified_block + 1;
                let proof = storage.load_proof(block_number);
                if let Ok(proof) = proof {
                    info!("New proof for block: {}", block_number);
                    let block = storage
                        .load_committed_block(block_number)
                        .unwrap_or_else(|| panic!("failed to load block #{}", block_number));
                    let op = Operation {
                        action: Action::Verify {
                            proof: Box::new(proof),
                        },
                        block,
                        accounts_updated: Vec::new(),
                        id: None,
                    };
                    let op = storage
                        .execute_operation(&op)
                        .expect("committer must commit the op into db");
                    tx_for_eth
                        .send(op)
                        .expect("must send an operation for verification to ethereum");
                    last_verified_block += 1;
                } else {
                    break;
                }
            }
        };
    }
}
