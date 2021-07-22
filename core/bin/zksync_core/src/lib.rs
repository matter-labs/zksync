use crate::register_factory_handler::run_register_factory_handler;
use crate::state_keeper::ZkSyncStateInitParams;
use crate::{
    block_proposer::run_block_proposer_task,
    committer::run_committer,
    eth_watch::start_eth_watch,
    mempool::run_mempool_tasks,
    private_api::start_private_core_api,
    rejected_tx_cleaner::run_rejected_tx_cleaner,
    state_keeper::{start_state_keeper, ZkSyncStateKeeper},
    token_handler::run_token_handler,
};
use futures::{channel::mpsc, future};
use tokio::task::JoinHandle;
use zksync_config::ZkSyncConfig;
use zksync_eth_client::EthereumGateway;
use zksync_gateway_watcher::run_gateway_watcher_if_multiplexed;
use zksync_storage::ConnectionPool;
use zksync_types::{tokens::get_genesis_token_list, Token, TokenId};

const DEFAULT_CHANNEL_CAPACITY: usize = 32_768;

pub mod block_proposer;
pub mod committer;
pub mod eth_watch;
pub mod mempool;
pub mod private_api;
pub mod register_factory_handler;
pub mod rejected_tx_cleaner;
pub mod state_keeper;
pub mod token_handler;

/// Waits for *any* of the tokio tasks to be finished.
/// Since the main tokio tasks are used as actors which should live as long
/// as application runs, any possible outcome (either `Ok` or `Err`) is considered
/// as a reason to stop the server completely.
pub async fn wait_for_tasks(task_futures: Vec<JoinHandle<()>>) {
    match future::select_all(task_futures).await {
        (Ok(_), _, _) => {
            panic!("One of the actors finished its run, while it wasn't expected to do it");
        }
        (Err(error), _, _) => {
            vlog::warn!("One of the tokio actors unexpectedly finished, shutting down");
            if error.is_panic() {
                // Resume the panic on the main task
                std::panic::resume_unwind(error.into_panic());
            }
        }
    }
}

/// Inserts the initial information about zkSync tokens into the database.
pub async fn genesis_init(config: &ZkSyncConfig) {
    let pool = ConnectionPool::new(Some(1));

    vlog::info!("Generating genesis block.");
    ZkSyncStateKeeper::create_genesis_block(
        pool.clone(),
        &config.chain.state_keeper.fee_account_addr,
    )
    .await;
    vlog::info!("Adding initial tokens to db");
    let genesis_tokens = get_genesis_token_list(&config.chain.eth.network.to_string())
        .expect("Initial token list not found");
    for (id, token) in (1..).zip(genesis_tokens) {
        vlog::info!(
            "Adding token: {}, id:{}, address: {}, decimals: {}",
            token.symbol,
            id,
            token.address,
            token.decimals
        );
        pool.access_storage()
            .await
            .expect("failed to access db")
            .tokens_schema()
            .store_token(Token {
                id: TokenId(id as u32),
                symbol: token.symbol,
                address: token.address,
                decimals: token.decimals,
                is_nft: false,
            })
            .await
            .expect("failed to store token");
    }
}

/// Starts the core application, which has the following sub-modules:
///
/// - Ethereum Watcher, module to monitor on-chain operations.
/// - zkSync state keeper, module to execute and seal blocks.
/// - mempool, module to organize incoming transactions.
/// - block proposer, module to create block proposals for state keeper.
/// - committer, module to store pending and completed blocks into the database.
/// - private Core API server.
pub async fn run_core(
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
    eth_gateway: EthereumGateway,
    config: &ZkSyncConfig,
) -> anyhow::Result<Vec<JoinHandle<()>>> {
    let (proposed_blocks_sender, proposed_blocks_receiver) =
        mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
    let (state_keeper_req_sender, state_keeper_req_receiver) =
        mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
    let (eth_watch_req_sender, eth_watch_req_receiver) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
    let (mempool_tx_request_sender, mempool_tx_request_receiver) =
        mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
    let (mempool_block_request_sender, mempool_block_request_receiver) =
        mpsc::channel(DEFAULT_CHANNEL_CAPACITY);

    // Start Ethereum Watcher.
    let eth_watch_task = start_eth_watch(
        eth_watch_req_sender.clone(),
        eth_watch_req_receiver,
        eth_gateway.clone(),
        &config,
    );

    // Insert pending withdrawals into database (if required)
    let mut storage_processor = connection_pool.access_storage().await?;

    // Start State Keeper.
    let state_keeper_init = ZkSyncStateInitParams::restore_from_db(&mut storage_processor).await?;
    let pending_block = state_keeper_init
        .get_pending_block(&mut storage_processor)
        .await;

    let state_keeper = ZkSyncStateKeeper::new(
        state_keeper_init,
        config.chain.state_keeper.fee_account_addr,
        state_keeper_req_receiver,
        proposed_blocks_sender,
        config.chain.state_keeper.block_chunk_sizes.clone(),
        config.chain.state_keeper.miniblock_iterations as usize,
        config.chain.state_keeper.fast_block_miniblock_iterations as usize,
        config.chain.state_keeper.last_tx_signer_data(),
    );
    let state_keeper_task = start_state_keeper(state_keeper, pending_block);

    // Start committer.
    let committer_task = run_committer(
        proposed_blocks_receiver,
        mempool_block_request_sender.clone(),
        connection_pool.clone(),
        &config,
    );

    // Start mempool.
    let mempool_task = run_mempool_tasks(
        connection_pool.clone(),
        mempool_tx_request_receiver,
        mempool_block_request_receiver,
        eth_watch_req_sender.clone(),
        &config,
        4,
        DEFAULT_CHANNEL_CAPACITY,
    );

    let gateway_watcher_task_opt = run_gateway_watcher_if_multiplexed(eth_gateway.clone(), &config);

    // Start token handler.
    let token_handler_task = run_token_handler(
        connection_pool.clone(),
        eth_watch_req_sender.clone(),
        &config,
    );

    // Start token handler.
    let register_factory_task = run_register_factory_handler(
        connection_pool.clone(),
        eth_watch_req_sender.clone(),
        &config,
    );
    // Start rejected transactions cleaner task.
    let rejected_tx_cleaner_task = run_rejected_tx_cleaner(&config, connection_pool.clone());

    // Start block proposer.
    let proposer_task = run_block_proposer_task(
        &config,
        mempool_block_request_sender.clone(),
        state_keeper_req_sender.clone(),
    );

    // Start private API.
    start_private_core_api(
        panic_notify.clone(),
        mempool_tx_request_sender,
        eth_watch_req_sender,
        config.api.private.clone(),
    );

    let mut task_futures = vec![
        eth_watch_task,
        state_keeper_task,
        committer_task,
        mempool_task,
        proposer_task,
        rejected_tx_cleaner_task,
        token_handler_task,
        register_factory_task,
    ];

    if let Some(task) = gateway_watcher_task_opt {
        task_futures.push(task);
    }

    Ok(task_futures)
}
