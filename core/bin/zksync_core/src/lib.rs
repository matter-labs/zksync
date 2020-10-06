use crate::eth_watch::EthWatchRequest;
use zksync_storage::StorageProcessor;
use zksync_types::tx::TxHash;

use crate::{
    block_proposer::run_block_proposer_task,
    committer::run_committer,
    eth_watch::start_eth_watch,
    mempool::run_mempool_task,
    private_api::start_private_core_api,
    state_keeper::{start_state_keeper, ZksyncStateInitParams, ZksyncStateKeeper},
};
use futures::{
    channel::{mpsc, oneshot},
    future, SinkExt, StreamExt,
};
use std::cell::RefCell;
use tokio::task::JoinHandle;
use zksync_config::ConfigurationOptions;
use zksync_storage::ConnectionPool;

pub mod block_proposer;
pub mod committer;
pub mod eth_watch;
pub mod mempool;
pub mod private_api;
pub mod state_keeper;

pub async fn insert_pending_withdrawals(
    storage: &mut StorageProcessor<'_>,
    eth_watch_req_sender: mpsc::Sender<EthWatchRequest>,
) {
    // Check if the pending withdrawals table is empty
    let no_stored_pending_withdrawals = storage
        .chain()
        .operations_schema()
        .no_stored_pending_withdrawals()
        .await
        .expect("failed to call no_stored_pending_withdrawals function");
    if no_stored_pending_withdrawals {
        let eth_watcher_channel = oneshot::channel();

        eth_watch_req_sender
            .clone()
            .send(EthWatchRequest::GetPendingWithdrawalsQueueIndex {
                resp: eth_watcher_channel.0,
            })
            .await
            .expect("EthWatchRequest::GetPendingWithdrawalsQueueIndex request sending failed");

        let pending_withdrawals_queue_index = eth_watcher_channel
            .1
            .await
            .expect("EthWatchRequest::GetPendingWithdrawalsQueueIndex response error")
            .expect("Err as result of EthWatchRequest::GetPendingWithdrawalsQueueIndex");

        // Let's add to the db one 'fake' pending withdrawal with
        // id equals to (pending_withdrawals_queue_index-1)
        // Next withdrawals will be added to the db with correct
        // corresponding indexes in contract's pending withdrawals queue
        storage
            .chain()
            .operations_schema()
            .add_pending_withdrawal(
                &TxHash::default(),
                Some(pending_withdrawals_queue_index as i64 - 1),
            )
            .await
            .expect("can't save fake pending withdrawal in the db");
    }
}

/// Waits for *any* of the tokio tasks to be finished.
/// Since the main tokio tasks are used as actors which should live as long
/// as application runs, any possible outcome (either `Ok` or `Err`) is considered
/// as a reason to stop the server completely.
async fn wait_for_tasks(task_futures: Vec<JoinHandle<()>>) {
    match future::select_all(task_futures).await {
        (Ok(_), _, _) => {
            panic!("One of the actors finished its run, while it wasn't expected to do it");
        }
        (Err(error), _, _) => {
            log::warn!("One of the tokio actors unexpectedly finished, shutting down");
            if error.is_panic() {
                // Resume the panic on the main task
                std::panic::resume_unwind(error.into_panic());
            }
        }
    }
}

/// Waits for a message on a `stop_signal_receiver`. This receiver exists
/// for threads that aren't using the tokio Runtime to run on, and thus
/// cannot be handled the same way as the tokio tasks.
async fn wait_for_stop_signal(mut stop_signal_receiver: mpsc::Receiver<bool>) {
    stop_signal_receiver.next().await;
}

/// Starts the core application, which has the following sub-modules:
///
/// - Ethereum Watcher, module to monitor on-chain operations.
/// - zkSync state keeper, module to execute and seal blocks.
/// - mempool, module to organize incoming transactions.
/// - block proposer, module to create block proposals for state keeper.
/// - committer, module to store pending and completed blocks into the database.
/// - private Core API server.
pub async fn run_core() -> anyhow::Result<()> {
    let connection_pool = ConnectionPool::new(None).await;
    let config_opts = ConfigurationOptions::from_env();

    let channel_size = 32768;

    let (proposed_blocks_sender, proposed_blocks_receiver) = mpsc::channel(channel_size);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(channel_size);
    let (eth_watch_req_sender, eth_watch_req_receiver) = mpsc::channel(channel_size);
    let (mempool_request_sender, mempool_request_receiver) = mpsc::channel(channel_size);

    // Handle ctrl+c
    let (stop_signal_sender, stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            futures::executor::block_on(sender.send(true)).expect("crtlc signal send");
        })
        .expect("Error setting Ctrl-C handler");
    }

    // Start Ethereum Watcher.
    let eth_watch_task = start_eth_watch(
        config_opts.clone(),
        eth_watch_req_sender.clone(),
        eth_watch_req_receiver,
        connection_pool.clone(),
    );

    // Insert pending withdrawals into database (if required)
    let mut storage_processor = connection_pool.access_storage().await?;
    insert_pending_withdrawals(&mut storage_processor, eth_watch_req_sender.clone()).await;

    // Start State Keeper.
    let state_keeper_init = ZksyncStateInitParams::restore_from_db(&mut storage_processor).await?;
    let pending_block = state_keeper_init
        .get_pending_block(&mut storage_processor)
        .await;
    let state_keeper = ZksyncStateKeeper::new(
        state_keeper_init,
        config_opts.operator_fee_eth_addr,
        state_keeper_req_receiver,
        proposed_blocks_sender,
        config_opts.available_block_chunk_sizes.clone(),
        config_opts.miniblock_timings.max_miniblock_iterations,
        config_opts.miniblock_timings.fast_miniblock_iterations,
        config_opts.max_number_of_withdrawals_per_block,
    );
    let state_keeper_task = start_state_keeper(state_keeper, pending_block);

    // Start committer.
    let committer_task = run_committer(
        proposed_blocks_receiver,
        mempool_request_sender.clone(),
        connection_pool.clone(),
    );

    // Start mempool.
    let mempool_task = run_mempool_task(
        connection_pool.clone(),
        mempool_request_receiver,
        eth_watch_req_sender,
        &config_opts,
    );

    // Start block proposer.
    let proposer_task = run_block_proposer_task(
        &config_opts,
        mempool_request_sender.clone(),
        state_keeper_req_sender.clone(),
    );

    // Start private API.
    start_private_core_api(config_opts, stop_signal_sender, mempool_request_sender);

    let task_futures = vec![
        eth_watch_task,
        state_keeper_task,
        committer_task,
        mempool_task,
        proposer_task,
    ];

    let task_future = wait_for_tasks(task_futures);
    let signal_future = wait_for_stop_signal(stop_signal_receiver);

    // Select either of futures: completion of the any will mean that
    // server has to be stopped.
    tokio::select! {
        _ = task_future => {
            // Do nothing, task future always panic upon finishing.
        },
        _ = signal_future => {
            log::warn!("Stop signal received, shutting down");
        },
    }

    Ok(())
}
