//! Mempool is buffer for transactions.
//!
//! The role is:
//! 1) Storing txs to the database
//! 2) Getting txs from database.
//! 3) When polled return vector of the transactions in the queue.
//!
//! For better consistency, we always store all txs in the database and get them only if they are requested.
//!
//! Communication channel with other actors:
//! Mempool does not push information to other actors, only accepts requests. (see `MempoolRequest`)

// External uses
use futures::channel::mpsc;

use tokio::task::JoinHandle;

// Workspace uses
use zksync_storage::ConnectionPool;

// Local uses
use crate::block_handler::MempoolBlocksHandler;
pub use crate::block_handler::{GetBlockRequest, MempoolBlocksRequest, ProposedBlock};
use crate::mempool_transactions_queue::MempoolTransactionsQueue;
use crate::state::MempoolState;
pub use crate::transactions_handler::MempoolTransactionRequest;
use crate::transactions_handler::MempoolTransactionsHandler;

mod block_handler;
mod mempool_transactions_queue;
mod state;
mod transactions_handler;

// Due channel based nature, for better performance,
// you need to run independent mempool_tx_handler for each actor, e.g. for each API actor
#[must_use]
pub fn run_mempool_tx_handler(
    db_pool: ConnectionPool,
    tx_requests: mpsc::Receiver<MempoolTransactionRequest>,
    block_chunk_sizes: Vec<usize>,
) -> JoinHandle<()> {
    let mempool_state = MempoolState::new(db_pool.clone());
    let max_block_size_chunks = *block_chunk_sizes
        .iter()
        .max()
        .expect("failed to find max block chunks size");
    let handler = MempoolTransactionsHandler {
        db_pool,
        mempool_state,
        requests: tx_requests,
        max_block_size_chunks,
    };
    tokio::spawn(handler.run())
}

#[must_use]
pub fn run_mempool_block_handler(
    db_pool: ConnectionPool,
    block_requests: mpsc::Receiver<MempoolBlocksRequest>,
) -> JoinHandle<()> {
    let mempool_state = MempoolState::new(db_pool);

    let blocks_handler = MempoolBlocksHandler {
        mempool_state,
        requests: block_requests,
    };

    tokio::spawn(blocks_handler.run())
}
