use std::sync::mpsc::{channel, Sender, Receiver};
use plasma::eth_client::{TxMeta};
use super::storage::{ConnectionPool, StorageProcessor};
use super::server_models::{Operation, Action, ProverRequest, CommitRequest};

pub fn run_committer(
    rx_for_ops: Receiver<CommitRequest>, 
    tx_for_eth: Sender<Operation>,
    tx_for_proof_requests: Sender<ProverRequest>,
    pool: ConnectionPool,
) {

    let storage = pool.access_storage().expect("db connection failed for committer");;

    // request unverified proofs
    let ops = storage.load_unverified_commitments().expect("committer must load pending ops from db");
    for op in ops {
        //let op: Operation = serde_json::from_value(pending_op.data).unwrap();
        if let Action::Commit = op.action {
            tx_for_proof_requests.send(ProverRequest(op.block.block_number, op.block)).expect("must send a proof request for pending operations");
        }
    }

    for mut req in rx_for_ops {
        let op = match req {
            CommitRequest::NewBlock{block, accounts_updated} => {
                Operation{
                    action: Action::Commit, 
                    block, 
                    accounts_updated: Some(accounts_updated), 
                    tx_meta: None
                }
            },
            CommitRequest::NewProof(block_number, block, proof) => {
                Operation{
                    action: Action::Verify{proof}, 
                    block, 
                    accounts_updated: None, 
                    tx_meta: None
                }
            },
        };

        // persist in storage first
        let op: Operation = storage.execute_operation(&op).expect("committer must commit the op into db");

        // send a request for proof
        if let Action::Commit = op.action {
            tx_for_proof_requests.send(ProverRequest(op.block.block_number, op.block.clone())).expect("must send a proof request");
        }

        // then submit to eth
        tx_for_eth.send(op).expect("must send an operation for commitment to ethereum");
    }
}
