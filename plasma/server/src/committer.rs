use std::sync::mpsc::{channel, Sender, Receiver};
use plasma::eth_client::{TxMeta};
use super::storage::{ConnectionPool, StorageProcessor};
use super::server_models::{Operation, Action, ProverRequest};

pub fn run_committer(
    rx_for_ops: Receiver<Operation>, 
    tx_for_eth: Sender<(Operation, TxMeta)>,
    tx_for_proof_requests: Sender<ProverRequest>,
    pool: ConnectionPool,
) {

    // scope is to avoid manual dropping of established storage
    {
        let connection = pool.pool.get().expect("committer must connect to db");
        let storage = StorageProcessor::from_connection(connection);

        // request unverified proofs
        let ops = storage.load_pendings_proof_reqs().expect("committer must load pending ops from db");
        for pending_op in ops {
            let op: Operation = serde_json::from_value(pending_op.data).unwrap();
            if let Action::Commit{block, new_root: _} = op.action {
                tx_for_proof_requests.send(ProverRequest(op.block_number, block.unwrap(), op.block_data.clone(), op.accounts_updated.clone())).expect("must send a proof request for pending operations");
            }
        }
    }

    for mut op in rx_for_ops {
        // persist in storage first
        let connection = pool.pool.get().expect("committer must connect to db");
        let storage = StorageProcessor::from_connection(connection);
        let committed_op = storage.commit_op(&op).expect("committer must commit the op into db");

        if let Action::Commit{ref mut block, new_root: _} = op.action {
            tx_for_proof_requests.send(ProverRequest(op.block_number, block.take().unwrap(), op.block_data.clone(), op.accounts_updated.clone()))
                .expect("must send a proof request");
        }

        // then submit to eth
        tx_for_eth.send((op, TxMeta{
            addr:   committed_op.addr, 
            nonce:  committed_op.nonce as u32,
        })).expect("must send an operation for commitment to ethereum");

    }
}
