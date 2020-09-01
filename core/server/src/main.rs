// Built-in deps
use std::cell::RefCell;
use std::time::Duration;
// External uses
use clap::{App, Arg};
use futures::{channel::mpsc, executor::block_on, future, SinkExt, StreamExt};
use tokio::{runtime::Runtime, task::JoinHandle};
use web3::types::H160;
// Workspace uses
use models::{
    config_options::{ConfigurationOptions, ProverOptions},
    node::{
        config::OBSERVER_MODE_PULL_INTERVAL,
        tokens::{get_genesis_token_list, Token},
        TokenId,
    },
};
use storage::ConnectionPool;
// Local uses
use server::prometheus_exporter::start_prometheus_exporter;
use server::{
    api_server::start_api_server,
    block_proposer::run_block_proposer_task,
    committer::run_committer,
    eth_sender,
    eth_watch::start_eth_watch,
    fee_ticker::run_ticker_task,
    leader_election,
    mempool::run_mempool_task,
    observer_mode,
    prover_server::start_prover_server,
    state_keeper::{start_state_keeper, PlasmaStateKeeper},
    utils::current_zksync_info::CurrentZksyncInfo,
};

fn main() {
    env_logger::init();

    let config_opts = ConfigurationOptions::from_env();

    let cli = App::new("zkSync operator node")
        .author("Matter Labs")
        .arg(
            Arg::with_name("genesis")
                .long("genesis")
                .help("Generate genesis block for the first contract deployment"),
        )
        .get_matches();

    if cli.is_present("genesis") {
        let pool = ConnectionPool::new(Some(1));
        log::info!("Generating genesis block.");
        PlasmaStateKeeper::create_genesis_block(pool.clone(), &config_opts.operator_fee_eth_addr);
        log::info!("Adding initial tokens to db");
        let genesis_tokens =
            get_genesis_token_list(&config_opts.eth_network).expect("Initial token list not found");
        for (id, token) in (1..).zip(genesis_tokens) {
            log::info!(
                "Adding token: {}, id:{}, address: {}, decimals: {}",
                token.symbol,
                id,
                token.address,
                token.decimals
            );
            pool.access_storage()
                .expect("failed to access db")
                .tokens_schema()
                .store_token(Token {
                    id: id as TokenId,
                    symbol: token.symbol,
                    address: token.address[2..]
                        .parse()
                        .expect("failed to parse token address"),
                    decimals: token.decimals,
                })
                .expect("failed to store token");
        }
        return;
    }

    // Start observing the state and try to become leader.
    let observer_mode_final_state = {
        let (observed_state_tx, observed_state_rx) = std::sync::mpsc::channel();
        let (stop_observer_mode_tx, stop_observer_mode_rx) = std::sync::mpsc::channel();
        let jh = std::thread::Builder::new()
            .name("Observer mode".to_owned())
            .spawn(move || {
                let state = observer_mode::run(
                    ConnectionPool::new(Some(1)),
                    OBSERVER_MODE_PULL_INTERVAL,
                    stop_observer_mode_rx,
                );
                observed_state_tx.send(state).expect("unexpected failure");
            })
            .expect("failed to start observer mode");
        leader_election::block_until_leader().expect("voting for leader fail");
        stop_observer_mode_tx.send(()).expect("unexpected failure");
        let observer_mode_final_state = observed_state_rx.recv().expect("unexpected failure");
        jh.join().unwrap();
        observer_mode_final_state
    };

    let connection_pool = ConnectionPool::new(None);

    log::debug!("starting server");

    let storage = connection_pool
        .access_storage()
        .expect("db connection failed for committer");
    let contract_addr: H160 = storage
        .config_schema()
        .load_config()
        .expect("can not load server_config")
        .contract_addr
        .expect("contract_addr empty in server_config")[2..]
        .parse()
        .expect("contract_addr in db wrong");
    if contract_addr != config_opts.contract_eth_addr {
        panic!(
            "Contract addresses mismatch! From DB = {}, from env = {}",
            contract_addr, config_opts.contract_eth_addr
        );
    }

    let current_zksync_info = CurrentZksyncInfo::new(&connection_pool);

    log::info!("starting actors");

    let mut main_runtime = Runtime::new().expect("main runtime start");

    // handle ctrl+c
    let (stop_signal_sender, stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("crtlc signal send");
        })
        .expect("Error setting Ctrl-C handler");
    }

    let (eth_watch_req_sender, eth_watch_req_receiver) = mpsc::channel(256);
    let eth_watch_task = start_eth_watch(
        config_opts.clone(),
        eth_watch_req_sender.clone(),
        eth_watch_req_receiver,
        &main_runtime,
    );

    let (proposed_blocks_sender, proposed_blocks_receiver) = mpsc::channel(256);
    let (state_keeper_req_sender, state_keeper_req_receiver) = mpsc::channel(256);
    let (executed_tx_notify_sender, executed_tx_notify_receiver) = mpsc::channel(256);
    let (mempool_request_sender, mempool_request_receiver) = mpsc::channel(256);
    let (ticker_request_sender, ticker_request_receiver) = mpsc::channel(512);

    // Load the most recent pending block from the database.
    let pending_block = observer_mode_final_state
        .state_keeper_init
        .get_pending_block(&storage);
    let state_keeper = PlasmaStateKeeper::new(
        observer_mode_final_state.state_keeper_init,
        config_opts.operator_fee_eth_addr,
        state_keeper_req_receiver,
        proposed_blocks_sender,
        executed_tx_notify_sender,
        config_opts.available_block_chunk_sizes.clone(),
        config_opts.max_miniblock_iterations,
        config_opts.max_miniblock_iterations_withdraw_block,
    );
    let state_keeper_task = start_state_keeper(state_keeper, pending_block, &main_runtime);

    let (eth_send_request_sender, eth_send_request_receiver) = mpsc::channel(256);
    let (zksync_commit_notify_sender, zksync_commit_notify_receiver) = mpsc::channel(256);
    let eth_sender_task = eth_sender::start_eth_sender(
        &main_runtime,
        connection_pool.clone(),
        zksync_commit_notify_sender.clone(), // eth sender sends only verify blocks notifications
        eth_send_request_receiver,
        config_opts.clone(),
        current_zksync_info.clone(),
    );

    let committer_task = run_committer(
        proposed_blocks_receiver,
        eth_send_request_sender.clone(),
        zksync_commit_notify_sender, // commiter sends only commit block notifications
        mempool_request_sender.clone(),
        connection_pool.clone(),
        &main_runtime,
    );
    start_api_server(
        zksync_commit_notify_receiver,
        connection_pool.clone(),
        stop_signal_sender.clone(),
        mempool_request_sender.clone(),
        executed_tx_notify_receiver,
        state_keeper_req_sender.clone(),
        eth_watch_req_sender.clone(),
        ticker_request_sender,
        config_opts.clone(),
        current_zksync_info,
    );

    let prover_options = ProverOptions::from_env();
    start_prover_server(
        connection_pool.clone(),
        prover_options.gone_timeout,
        prover_options.prepare_data_interval,
        stop_signal_sender,
        config_opts.clone(),
    );

    let mempool_task = run_mempool_task(
        connection_pool.clone(),
        mempool_request_receiver,
        eth_watch_req_sender,
        &config_opts,
        &main_runtime,
    );
    let proposer_task = run_block_proposer_task(
        mempool_request_sender,
        state_keeper_req_sender.clone(),
        &main_runtime,
    );

    let ticker_task = run_ticker_task(
        config_opts.token_price_source.clone(),
        connection_pool.clone(),
        eth_send_request_sender,
        state_keeper_req_sender,
        ticker_request_receiver,
        &main_runtime,
    );

    let prometheus_exporter =
        start_prometheus_exporter(connection_pool, &config_opts, &main_runtime);

    let task_futures = vec![
        eth_watch_task,
        state_keeper_task,
        eth_sender_task,
        committer_task,
        mempool_task,
        proposer_task,
        ticker_task,
        prometheus_exporter,
    ];

    main_runtime.block_on(async move {
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
    });
    main_runtime.shutdown_timeout(Duration::from_secs(0));
}
