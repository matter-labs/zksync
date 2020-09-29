// Built-in uses
use std::time::Duration;
// External uses
use failure::format_err;
use futures::channel::mpsc::{Receiver, Sender};
use futures::{SinkExt, StreamExt};
use tokio::{task::JoinHandle, time};
// Workspace uses
use crate::eth_sender::ETHSenderRequest;
use crate::mempool::MempoolRequest;
use models::{
    block::{Block, ExecutedOperations, PendingBlock},
    AccountUpdates, Action, BlockNumber, Operation,
};
use storage::ConnectionPool;

#[derive(Debug)]
pub enum CommitRequest {
    PendingBlock((PendingBlock, AppliedUpdatesRequest)),
    Block((BlockCommitRequest, AppliedUpdatesRequest)),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlockCommitRequest {
    pub block: Block,
    pub accounts_updated: AccountUpdates,
}

#[derive(Clone, Debug)]
pub struct AppliedUpdatesRequest {
    pub account_updates: AccountUpdates,
    pub first_update_order_id: usize,
}

pub struct ExecutedOpsNotify {
    pub operations: Vec<ExecutedOperations>,
    pub block_number: BlockNumber,
}

const PROOF_POLL_INTERVAL: Duration = Duration::from_secs(1);

async fn handle_new_commit_task(
    mut rx_for_ops: Receiver<CommitRequest>,
    mut tx_for_eth: Sender<ETHSenderRequest>,
    mut op_notify_sender: Sender<Operation>,
    mut mempool_req_sender: Sender<MempoolRequest>,
    mut executed_tx_notify_sender: Sender<ExecutedOpsNotify>,
    pool: ConnectionPool,
) {
    while let Some(request) = rx_for_ops.next().await {
        match request {
            CommitRequest::Block((block_commit_request, applied_updates_req)) => {
                let operations = block_commit_request.block.block_transactions.clone();
                let block_number = block_commit_request.block.block_number;

                commit_block(
                    block_commit_request,
                    applied_updates_req,
                    &pool,
                    &mut tx_for_eth,
                    &mut op_notify_sender,
                    &mut mempool_req_sender,
                )
                .await;

                executed_tx_notify_sender
                    .send(ExecutedOpsNotify {
                        operations,
                        block_number,
                    })
                    .await
                    .map_err(|e| warn!("Failed to send executed tx notify batch: {}", e))
                    .unwrap_or_default();
            }
            CommitRequest::PendingBlock((pending_block, applied_updates_req)) => {
                let mut operations = pending_block.success_operations.clone();
                operations.extend(
                    pending_block
                        .failed_txs
                        .clone()
                        .into_iter()
                        .map(|tx| ExecutedOperations::Tx(Box::new(tx))),
                );
                let block_number = pending_block.number;

                save_pending_block(pending_block, applied_updates_req, &pool).await;

                executed_tx_notify_sender
                    .send(ExecutedOpsNotify {
                        operations,
                        block_number,
                    })
                    .await
                    .map_err(|e| warn!("Failed to send executed tx notify batch: {}", e))
                    .unwrap_or_default();
            }
        }
    }
}

async fn save_pending_block(
    pending_block: PendingBlock,
    applied_updates_request: AppliedUpdatesRequest,
    pool: &ConnectionPool,
) {
    let mut storage = pool
        .access_storage()
        .await
        .expect("db connection fail for committer");

    let mut transaction = storage
        .start_transaction()
        .await
        .expect("Failed initializing a DB transaction");

    let block_number = pending_block.number;

    log::trace!("persist pending block #{}", block_number);

    transaction
        .chain()
        .block_schema()
        .save_pending_block(pending_block)
        .await
        .expect("committer must commit the pending block into db");

    transaction
        .chain()
        .state_schema()
        .commit_state_update(
            block_number,
            &applied_updates_request.account_updates,
            applied_updates_request.first_update_order_id,
        )
        .await
        .expect("committer must commit the pending block into db");

    transaction
        .commit()
        .await
        .expect("Unable to commit DB transaction");
}

async fn commit_block(
    block_commit_request: BlockCommitRequest,
    applied_updates_request: AppliedUpdatesRequest,
    pool: &ConnectionPool,
    tx_for_eth: &mut Sender<ETHSenderRequest>,
    op_notify_sender: &mut Sender<Operation>,
    mempool_req_sender: &mut Sender<MempoolRequest>,
) {
    let BlockCommitRequest {
        block,
        accounts_updated,
    } = block_commit_request;

    let mut storage = pool
        .access_storage()
        .await
        .expect("db connection fail for committer");

    let mut transaction = storage
        .start_transaction()
        .await
        .expect("Failed initializing a DB transaction");

    for exec_op in block.block_transactions.clone() {
        if let Some(exec_tx) = exec_op.get_executed_tx() {
            if exec_tx.success && exec_tx.signed_tx.tx.is_withdraw() {
                transaction
                    .chain()
                    .operations_schema()
                    .add_pending_withdrawal(&exec_tx.signed_tx.tx.hash(), None)
                    .await
                    .map_err(|e| {
                        format_err!(
                            "Failed to save pending withdrawal {:?}, error : {}",
                            exec_tx,
                            e
                        )
                    })
                    .expect("failed to save pending withdrawals into db");
            }
        }
    }

    transaction
        .chain()
        .state_schema()
        .commit_state_update(
            block.block_number,
            &applied_updates_request.account_updates,
            applied_updates_request.first_update_order_id,
        )
        .await
        .expect("committer must commit the pending block into db");

    let op = Operation {
        action: Action::Commit,
        block: block.clone(),
        accounts_updated,
        id: None,
    };
    info!("commit block #{}", op.block.block_number);
    let op = transaction
        .chain()
        .block_schema()
        .execute_operation(Operation {
            action: Action::Commit,
            block,

            // Block schema shouldn't save account_updates in the db for
            // commit because we do that iteratively in the statekeeper
            accounts_updated: Vec::new(),

            id: None,
        })
        .await
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

    transaction
        .commit()
        .await
        .expect("Unable to commit DB transaction");
}

async fn poll_for_new_proofs_task(mut tx_for_eth: Sender<ETHSenderRequest>, pool: ConnectionPool) {
    let mut last_verified_block = {
        let mut storage = pool
            .access_storage()
            .await
            .expect("db connection failed for committer");
        storage
            .chain()
            .block_schema()
            .get_last_verified_block()
            .await
            .expect("db failed")
    };

    let mut timer = time::interval(PROOF_POLL_INTERVAL);
    loop {
        timer.tick().await;

        let mut storage = pool
            .access_storage()
            .await
            .expect("db connection failed for committer");

        loop {
            let block_number = last_verified_block + 1;
            let proof = storage.prover_schema().load_proof(block_number).await;
            if let Ok(Some(proof)) = proof {
                let mut transaction = storage
                    .start_transaction()
                    .await
                    .expect("Unable to start DB transaction");

                info!("New proof for block: {}", block_number);
                let block = transaction
                    .chain()
                    .block_schema()
                    .load_committed_block(block_number)
                    .await
                    .unwrap_or_else(|| panic!("failed to load block #{}", block_number));

                let op = Operation {
                    action: Action::Verify {
                        proof: Box::new(proof),
                    },
                    block,
                    accounts_updated: Vec::new(),
                    id: None,
                };
                let op = transaction
                    .chain()
                    .block_schema()
                    .execute_operation(op.clone())
                    .await
                    .expect("committer must commit the op into db");
                tx_for_eth
                    .send(ETHSenderRequest::SendOperation(op))
                    .await
                    .expect("must send an operation for verification to ethereum");
                last_verified_block += 1;

                transaction
                    .commit()
                    .await
                    .expect("Failed to commit transaction");
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
    executed_tx_notify_sender: Sender<ExecutedOpsNotify>,
    pool: ConnectionPool,
) -> JoinHandle<()> {
    tokio::spawn(handle_new_commit_task(
        rx_for_ops,
        tx_for_eth.clone(),
        op_notify_sender,
        mempool_req_sender,
        executed_tx_notify_sender,
        pool.clone(),
    ));
    tokio::spawn(poll_for_new_proofs_task(tx_for_eth, pool))
}
