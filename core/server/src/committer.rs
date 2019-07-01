use eth_client::ETHClient;
use models::abi::TEST_PLASMA_ALWAYS_VERIFY;
use models::{Action, CommitRequest, Operation};
use std::sync::mpsc::{Receiver, Sender};
use std::thread;
use std::time::Duration;
use storage::ConnectionPool;

pub fn start_committer(
    rx_for_ops: Receiver<CommitRequest>,
    tx_for_eth: Sender<Operation>,
    pool: ConnectionPool,
) {
    thread::Builder::new()
        .name("committer".to_string())
        .spawn(move || {
            run_committer(rx_for_ops, tx_for_eth, pool);
        })
        .expect("thread creation failed");
}

fn run_committer(
    rx_for_ops: Receiver<CommitRequest>,
    tx_for_eth: Sender<Operation>,
    pool: ConnectionPool,
) {
    info!("committer started");
    let storage = pool
        .access_storage()
        .expect("db connection failed for committer");;
    let mempool = pool
        .access_mempool()
        .expect("db connection failed for commiter");

    let eth_client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);
    let current_nonce = eth_client.current_nonce().expect("can not get nonce");
    let _ = storage.prepare_nonce_scheduling(&eth_client.current_sender(), current_nonce);

    let mut last_verified_block = storage.get_last_verified_block().expect("db failed");
    loop {
        let req = rx_for_ops.recv_timeout(Duration::from_millis(100));
        if let Ok(CommitRequest {
            block,
            accounts_updated,
            mut txs_executed,
        }) = req
        {
            let op = Operation {
                action: Action::Commit,
                block,
                accounts_updated: Some(accounts_updated),
                tx_meta: None,
                id: None,
            };
            info!("commit block #{}", op.block.block_number);
            let op = storage
                .execute_operation(&op)
                .expect("committer must commit the op into db");

            mempool
                .remove_txs(&txs_executed)
                .expect("commiter mempool fail");
            //tx_for_proof_requests.send(ProverRequest(op.block.block_number)).expect("must send a proof request");
            tx_for_eth
                .send(op)
                .expect("must send an operation for commitment to ethereum");
            continue;
        } else {
            // there was a timeout, so check for the new ready proofs
            loop {
                let block_number = last_verified_block + 1;
                let proof = storage.load_proof(block_number);
                if let Ok(proof) = proof {
                    let block = storage
                        .load_committed_block(block_number)
                        .unwrap_or_else(|| panic!("failed to load block #{}", block_number));
                    let op = Operation {
                        action: Action::Verify {
                            proof: Box::new(proof),
                        },
                        block,
                        accounts_updated: None,
                        tx_meta: None,
                        id: None,
                    };
                    let op = storage
                        .execute_operation(&op)
                        .expect("committer must commit the op into db");
                    tx_for_eth
                        .send(op)
                        .expect("must send an operation for commitment to ethereum");
                    last_verified_block += 1;
                } else {
                    break;
                }
            }
        };
    }
}
