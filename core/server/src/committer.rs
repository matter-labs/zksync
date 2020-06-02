// Built-in uses
use std::time::Duration;
// External uses
use futures::channel::mpsc::{Receiver, Sender};
use futures::{SinkExt, StreamExt};
use tokio::{runtime::Runtime, task::JoinHandle, time};
// Workspace uses
use crate::eth_sender::ETHSenderRequest;
use crate::mempool::MempoolRequest;
use models::{node::block::PendingBlock, Action, BlockCommitRequest, CommitRequest, Operation};
use storage::ConnectionPool;

const PROOF_POLL_INTERVAL: Duration = Duration::from_secs(1);

async fn handle_new_commit_task(
    mut rx_for_ops: Receiver<CommitRequest>,
    mut tx_for_eth: Sender<ETHSenderRequest>,
    mut op_notify_sender: Sender<Operation>,
    mut mempool_req_sender: Sender<MempoolRequest>,
    pool: ConnectionPool,
) {
    while let Some(request) = rx_for_ops.next().await {
        match request {
            CommitRequest::Block(request, notifier) => {
                commit_block(
                    request,
                    &pool,
                    &mut tx_for_eth,
                    &mut op_notify_sender,
                    &mut mempool_req_sender,
                )
                .await;

                notifier.send(()).expect("state keeper receiver dropped");
            }
            CommitRequest::PendingBlock(pending_block, notifier) => {
                save_pending_block(pending_block, &pool);

                notifier.send(()).expect("state keeper receiver dropped");
            }
        }
    }
}

fn save_pending_block(pending_block: PendingBlock, pool: &ConnectionPool) {
    let storage = pool
        .access_storage()
        .expect("db connection fail for committer");

    log::trace!("persist pending block #{}", pending_block.number);

    storage
        .chain()
        .block_schema()
        .save_pending_block(pending_block)
        .expect("committer must commit the pending block into db");
}

async fn commit_block(
    request: BlockCommitRequest,
    pool: &ConnectionPool,
    tx_for_eth: &mut Sender<ETHSenderRequest>,
    op_notify_sender: &mut Sender<Operation>,
    mempool_req_sender: &mut Sender<MempoolRequest>,
) {
    let BlockCommitRequest {
        block,
        accounts_updated,
    } = request;

    let storage = pool
        .access_storage()
        .expect("db connection fail for committer");

    // handle empty block case (only failed txs)
    if accounts_updated.is_empty() && block.number_of_processed_prior_ops() == 0 {
        info!(
            "Failed transactions committed block: #{}",
            block.block_number
        );
        storage
            .chain()
            .block_schema()
            .save_block_transactions(block.block_number, block.block_transactions)
            .expect("committer failed tx save");
        return;
    }

    let op = Operation {
        action: Action::Commit,
        block,
        accounts_updated,
        id: None,
    };
    info!("commit block #{}", op.block.block_number);
    let op = storage
        .chain()
        .block_schema()
        .execute_operation(op.clone())
        .expect("committer must commit the op into db");

    tx_for_eth
        .send(ETHSenderRequest::SendOperation(op.clone()))
        .await
        .expect("must send an operation for commitment to ethereum");

    // we notify about commit operation as soon as it is executed, we don't wait for eth confirmations
    op_notify_sender
        .send(op.clone())
        .await
        .map_err(|e| warn!("Failed notify about commit op confirmation: {}", e))
        .unwrap_or_default();

    mempool_req_sender
        .send(MempoolRequest::UpdateNonces(op.accounts_updated))
        .await
        .map_err(|e| warn!("Failed notify mempool about account updates: {}", e))
        .unwrap_or_default();
}

async fn poll_for_new_proofs_task(mut tx_for_eth: Sender<ETHSenderRequest>, pool: ConnectionPool) {
    let mut last_verified_block = {
        let storage = pool
            .access_storage()
            .expect("db connection failed for committer");
        storage
            .chain()
            .block_schema()
            .get_last_verified_block()
            .expect("db failed")
    };

    let mut timer = time::interval(PROOF_POLL_INTERVAL);
    loop {
        timer.tick().await;

        let storage = pool
            .access_storage()
            .expect("db connection failed for committer");

        loop {
            let block_number = last_verified_block + 1;
            let proof = storage.prover_schema().load_proof(block_number);
            if let Ok(proof) = proof {
                info!("New proof for block: {}", block_number);
                let block = storage
                    .chain()
                    .block_schema()
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
                    .chain()
                    .block_schema()
                    .execute_operation(op.clone())
                    .expect("committer must commit the op into db");
                tx_for_eth
                    .send(ETHSenderRequest::SendOperation(op))
                    .await
                    .expect("must send an operation for verification to ethereum");
                last_verified_block += 1;
            } else {
                break;
            }
        }
    }
}

#[must_use]
pub fn run_committer(
    rx_for_ops: Receiver<CommitRequest>,
    tx_for_eth: Sender<ETHSenderRequest>,
    op_notify_sender: Sender<Operation>,
    mempool_req_sender: Sender<MempoolRequest>,
    pool: ConnectionPool,
    runtime: &Runtime,
) -> JoinHandle<()> {
    runtime.spawn(handle_new_commit_task(
        rx_for_ops,
        tx_for_eth.clone(),
        op_notify_sender,
        mempool_req_sender,
        pool.clone(),
    ));
    runtime.spawn(poll_for_new_proofs_task(tx_for_eth, pool))
}
