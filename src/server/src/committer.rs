use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::time::Duration;
use eth_client::{ETHClient, TxMeta, TEST_PLASMA_ALWAYS_VERIFY};
use super::storage::{ConnectionPool, StorageProcessor};
use super::models::{Operation, Action, ProverRequest, CommitRequest};

pub fn start_committer(
    rx_for_ops: Receiver<CommitRequest>, 
    tx_for_eth: Sender<Operation>,
    pool: ConnectionPool,
) {
    thread::Builder::new().name("committer".to_string()).spawn(move || {
        run_committer(rx_for_ops, tx_for_eth, pool);
    }).expect("thread creation failed");
}

fn run_committer(
    rx_for_ops: Receiver<CommitRequest>, 
    tx_for_eth: Sender<Operation>,
    pool: ConnectionPool,
) {

    println!("committer started");
    let storage = pool.access_storage().expect("db connection failed for committer");;

    let mut eth_client = ETHClient::new(TEST_PLASMA_ALWAYS_VERIFY);
    let current_nonce = eth_client.current_nonce().expect("can not get nonce");
    storage.prepare_nonce_scheduling(&eth_client.current_sender(), current_nonce);

    let mut last_verified_block = storage.get_last_verified_block().expect("db failed");
    loop {
        let req = rx_for_ops.recv_timeout(Duration::from_millis(100));
        if let Ok(CommitRequest{block, accounts_updated}) = req {
            let op = Operation{
                action: Action::Commit, 
                block, 
                accounts_updated: Some(accounts_updated), 
                tx_meta: None,
                id:      None,
            };
            println!("commit block #{}", op.block.block_number);
            let op = storage.execute_operation(&op).expect("committer must commit the op into db");
            //tx_for_proof_requests.send(ProverRequest(op.block.block_number)).expect("must send a proof request");
            tx_for_eth.send(op).expect("must send an operation for commitment to ethereum");
            continue;
        } else {
            // there was a timeout, so check for the new ready proofs
            loop {
                let block_number = last_verified_block + 1;
                let proof = storage.load_proof(block_number);
                if let Ok(proof) = proof {
                    let block = storage.load_committed_block(block_number).expect(format!("failed to load block #{}", block_number).as_str());
                    let op = Operation{
                        action: Action::Verify{proof}, 
                        block, 
                        accounts_updated: None, 
                        tx_meta: None,
                        id: None
                    };
                    let op = storage.execute_operation(&op).expect("committer must commit the op into db");
                    tx_for_eth.send(op).expect("must send an operation for commitment to ethereum");
                    last_verified_block += 1;
                } else {
                    break;
                }
            }
        };
    }
}
