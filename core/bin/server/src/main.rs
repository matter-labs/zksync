use futures::{
    channel::mpsc,
    executor::block_on,
    SinkExt,
    StreamExt,
};
use std::{
    cell::RefCell,
    str::FromStr,
};

use structopt::StructOpt;
use serde::{Deserialize, Serialize};

use zksync_api::fee_ticker::{run_updaters, FeeTicker, TickerInfo};
use zksync_core::{
    genesis_init,
    run_core,
    wait_for_tasks,
};
use zksync_eth_client::EthereumGateway;
use zksync_forced_exit_requests::run_forced_exit_requests_actors;
use zksync_gateway_watcher::run_gateway_watcher_if_multiplexed;
use zksync_witness_generator::run_prover_server;

use tokio::task::JoinHandle;
use zksync_config::{
    configs::{
        api::{
            CommonApiConfig,
            JsonRpcConfig,
            PrivateApiConfig,
            ProverApiConfig,
            PrometheusConfig,
            RestApiConfig,
            TokenConfig,
            Web3Config,
        },
        ChainConfig,
        ContractsConfig,
        DBConfig,
        ETHClientConfig,
        ETHSenderConfig,
        ETHWatchConfig,
        ForcedExitRequestsConfig,
        GatewayWatcherConfig,
        ProverConfig,
        TickerConfig,
        ZkSyncConfig,
    },
};
use zksync_core::rejected_tx_cleaner::run_rejected_tx_cleaner;
use zksync_mempool::run_mempool_tx_handler;
use zksync_prometheus_exporter::{
    run_operation_counter,
    run_prometheus_exporter,
};
use zksync_storage::ConnectionPool;

const DEFAULT_CHANNEL_CAPACITY: usize = 32_768;

#[derive(Debug, Clone, Copy)]
pub enum ServerCommand {
    Genesis,
    Launch,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Component {
    // Api components
    RestApi,
    Web3Api,
    RpcApi,
    RpcWebSocketApi,

    // Core components
    Fetchers,
    EthSender,
    Core,
    WitnessGenerator,
    ForcedExit,

    // Additional components
    Prometheus,
    PrometheusPeriodicMetrics,
    RejectedTaskCleaner,
}

impl FromStr for Component {
    type Err = String;

    fn from_str(s: &str) -> Result<Component, String> {
        // Convert the given string to a `Component` value.
        // If the string is not a valid component name, return an error.
        match s {
            "rest-api" => Ok(Component::RestApi),
            "web3-api" => Ok(Component::Web3Api),
            "rpc-api" => Ok(Component::RpcApi),
            "rpc-websocket-api" => Ok(Component::RpcWebSocketApi),
            "eth-sender" => Ok(Component::EthSender),
            "witness-generator" => Ok(Component::WitnessGenerator),
            "forced-exit" => Ok(Component::ForcedExit),
            "prometheus" => Ok(Component::Prometheus),
            "fetchers" => Ok(Component::Fetchers),
            "core" => Ok(Component::Core),
            "rejected-task-cleaner" => Ok(Component::RejectedTaskCleaner),
            "prometheus-periodic-metrics" => Ok(Component::PrometheusPeriodicMetrics),
            other => Err(format!("{} is not a valid component name", other)),
        }
    }
}

#[derive(Debug)]
struct ComponentsToRun(Vec<Component>);

impl Default for ComponentsToRun {
    fn default() -> Self {
        Self(vec![
            Component::RestApi,
            Component::Web3Api,
            Component::RpcApi,
            Component::RpcWebSocketApi,
            Component::EthSender,
            Component::WitnessGenerator,
            Component::ForcedExit,
            Component::Prometheus,
            Component::Core,
            Component::RejectedTaskCleaner,
            Component::Fetchers,
            Component::PrometheusPeriodicMetrics,
        ])
    }
}

impl FromStr for ComponentsToRun {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Parse the given string as a list of components separated by commas.
        // For each component name, convert it to a `Component` value.
        // If any of the component names are invalid, return an error.
        // Otherwise, return the parsed list of components.
        Ok(Self(
            s.split(',')
                .map(|x| Component::from_str(x.trim()))
                .collect::<Result<Vec<Component>, String>>()?,
        ))
    }
}

#[derive(StructOpt)]
#[structopt(name = "zkSync operator node", author = "Matter Labs")]
struct Opt {
    /// Generate genesis block for the first contract deployment
    #[structopt(long)]
    genesis: bool,
    /// comma-separated list of components to launch
    #[structopt(
    long,
    default_value = "rest-api,web3-api,rpc-api,rpc-websocket-api,eth-sender,witness-generator,forced-exit,prometheus,core,rejected-task-cleaner,fetchers,prometheus-periodic-metrics"
    )]
    components: ComponentsToRun,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Parse the command-line arguments and get the server mode (`genesis` or `launch`).
    let opt = Opt::from_args();
    let mut _vlog_guard = None;
    let server_mode = if opt.genesis {
        ServerCommand::Genesis
    } else {
        // Initialize the vlog logger if the `launch` mode is selected.
        _vlog_guard = Some(vlog::init());
        ServerCommand::Launch
    };

    if let ServerCommand::Genesis = server_mode {
        // If the `genesis` mode is selected, perform the server genesis initialization.
        vlog::info!("Performing the server genesis initialization",);
        let config = ChainConfig::from_env();
        genesis_init(&config).await;
        return Ok(());
    }

    // It's a `ServerCommand::Launch`, perform the usual routine.
    vlog::info!("Running the zkSync server");

    // Run the server with the specified components.
    run_server(&opt.components).await;

    Ok(())
}

async fn run_server(components: &ComponentsToRun) {
    // Initialize a connection pool for the database.
    let connection_pool = ConnectionPool::new(None);

    // Initialize a read-only connection pool for the database.
    let read_only_connection_pool = ConnectionPool::new_readonly_pool(None);

    // Initialize a sender and receiver for the stop signal.
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);

    // Create an empty vector to store the tasks that are launched.
    let mut tasks = vec![];

    // Check if the web3 API component is included in the components to run.
    if components.0.contains(&Component::Web3Api) {
        // Run the web3 API server.
        tasks.push(zksync_api::api_server::web3::start_rpc_server(
            connection_pool.clone(),
            &Web3Config::from_env(),
            &TokenConfig::from_env(),
        ));
    }

    // Check if the fetchers component is included in the components to run.
    if components.0.contains(&Component::Fetchers) {
        // Run the price fetchers.
        let mut price_tasks = run_price_updaters(connection_pool.clone());
        tasks.append(&mut price_tasks);
    }

    // Check if any of the RPC or REST API components are included in the components to run.
    if components.0.iter().any(|c| {
        matches!(
            c,
            Component::RpcWebSocketApi | Component::RpcApi | Component::RestApi
        )
    }) {
        // Create an Ethereum gateway.
        let eth_gateway = create_eth_gateway();

        // Load the Ethereum watch configuration from the environment.
        let eth_watch_config = ETHWatchConfig::from_env();

        // Load the gateway watcher configuration from the environment.
        let gateway_watcher_config = GatewayWatcherConfig::from_env();

        // Check if the eth multiplexer component is included in the components to run.
        if let Some(task) =
            run_gateway_watcher_if_multiplexed(eth_gateway.clone(), &gateway_watcher_config)
        {
            // Run the eth multiplexer.
            tasks.push(task);
        }

        // Initialize a sender and receiver for the sign check messages.
        let (sign_check_sender, sign_check_receiver) = mpsc::channel(DEFAULT_CHANNEL_CAPACITY);

        // Run the signer.
        tasks.push(zksync_api::signature_checker::start_sign_checker(
            eth_gateway,
            sign_check_receiver,
        ));

        // Load the contracts configuration from the environment.
        let contracts_config = ContractsConfig::from_env();

        // Load the common API configuration from the environment.
        let common_config = CommonApiConfig::from_env();

        // Load the token configuration from the environment.
        let token_config = TokenConfig::from_env();
        let chain_config = ChainConfig::from_env();
        let fee_ticker_config = TickerConfig::from_env();

        // Create a ticker info instance with the given connection pool.
        let ticker_info = Box::new(TickerInfo::new(read_only_connection_pool.clone()));

        // Create a new `FeeTicker` instance with the given configuration and connection pool.
        let ticker = FeeTicker::new_with_default_validator(
            ticker_info,
            fee_ticker_config,
            chain_config.max_blocks_to_aggregate(),
            read_only_connection_pool.clone(),
        );

        // If the RPC WebSocket API component is set to run,
        // create a channel for the mempool transactions and run the API server.
        if components.0.contains(&Component::RpcWebSocketApi) {
            let (mempool_tx_request_sender, mempool_tx_request_receiver) =
                mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
            tasks.push(run_mempool_tx_handler(
                connection_pool.clone(),
                mempool_tx_request_receiver,
                chain_config.state_keeper.block_chunk_sizes.clone(),
            ));
            tasks.push(zksync_api::api_server::rpc_subscriptions::start_ws_server(
                read_only_connection_pool.clone(),
                sign_check_sender.clone(),
                ticker.clone(),
                &common_config,
                &token_config,
                &JsonRpcConfig::from_env(),
                chain_config.state_keeper.miniblock_iteration_interval(),
                mempool_tx_request_sender,
                eth_watch_config.confirmations_for_eth_event,
            ));
        }

        // If the RPC API component is set to run,
        // create a channel for the mempool transactions and run the API server.
        if components.0.contains(&Component::RpcApi) {
            let (mempool_tx_request_sender, mempool_tx_request_receiver) =
                mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
            tasks.push(run_mempool_tx_handler(
                connection_pool.clone(),
                mempool_tx_request_receiver,
                chain_config.state_keeper.block_chunk_sizes.clone(),
            ));
            tasks.push(zksync_api::api_server::rpc_server::start_rpc_server(
                read_only_connection_pool.clone(),
                sign_check_sender.clone(),
                ticker.clone(),
                &JsonRpcConfig::from_env(),
                &common_config,
                &token_config,
                mempool_tx_request_sender,
                eth_watch_config.confirmations_for_eth_event,
            ));
        }

        // If the REST API component is set to run,
        // create a channel for the mempool transactions and run the API server.
        if components.0.contains(&Component::RestApi) {
            let (mempool_tx_request_sender, mempool_tx_request_receiver) =
                mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
            tasks.push(run_mempool_tx_handler(
                connection_pool.clone(),
                mempool_tx_request_receiver,
                chain_config.state_keeper.block_chunk_sizes,
            ));
            let private_config = PrivateApiConfig::from_env();
            tasks.push(zksync_api::api_server::rest::start_server_thread_detached(
                read_only_connection_pool.clone(),
                connection_pool.clone(),
                RestApiConfig::from_env().bind_addr(),
                contracts_config.contract_addr,
                ticker,
                sign_check_sender,
                mempool_tx_request_sender,
                private_config.url,
            ));
        }
    }

    // If the Ethereum sender component is set to run,
    // start the Ethereum sender.
    if components.0.contains(&Component::EthSender) {
        tasks.push(run_eth_sender(connection_pool.clone()))
    }

    // If the Core component is set to run,
    // create an Ethereum gateway and start the Core.
    if components.0.contains(&Component::Core) {
        let eth_gateway = create_eth_gateway();

        tasks.append(
            &mut run_core(
                connection_pool.clone(),
                read_only_connection_pool.clone(),
                &ZkSyncConfig::from_env(),
                eth_gateway.clone(),
            )
                .await
                .unwrap(),
        );
    }

    // If the WitnessGenerator component is set to run,
    // start the WitnessGenerator.
    if components.0.contains(&Component::WitnessGenerator) {
        tasks.push(run_witness_generator(connection_pool.clone()))
    }

    // If the Prometheus component is set to run,
    // start the Prometheus data exporter.
    // Also, if the PrometheusPeriodicMetrics component is set to run,
    // start the Prometheus operation counter.
    if components.0.contains(&Component::Prometheus) {
        let config = PrometheusConfig::from_env();
        let prometheus_task_handle = run_prometheus_exporter(config.port);
        tasks.push(prometheus_task_handle);
        if components.0.contains(&Component::PrometheusPeriodicMetrics) {
            let counter_task_handle = run_operation_counter(read_only_connection_pool.clone());
            tasks.push(counter_task_handle);
        }
    }

    // If the ForcedExit component is set to run,
    // start the ForcedExit.
    if components.0.contains(&Component::ForcedExit) {
        tasks.append(&mut run_forced_exit(connection_pool.clone()));
    }

    // If the RejectedTaskCleaner component is set to run,
    // start the RejectedTaskCleaner.
    if components.0.contains(&Component::RejectedTaskCleaner) {
        let config = DBConfig::from_env();
        tasks.push(run_rejected_tx_cleaner(&config, connection_pool));
    }

    // Set up a Ctrl+C handler that sends a stop signal to the server.
    // The stop signal is sent through a mpsc channel.
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("Ctrl+C signal send");
        })
            .expect("Error setting Ctrl+C handler");
    }

    // Wait for either the stop signal to be received or
    // for one of the tasks in the `tasks` vector to finish.
    // If one of the tasks doesn't finish, print a panic message.
    // If the stop signal is received, print a warning message.
    tokio::select! {
        _ = async { wait_for_tasks(tasks).await } => {
            panic!("One if the actors is not supposed to finish its execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            vlog::warn!("Stop signal received, shutting down");
        }
    }
}

// Run the ForcedExitRequests actors.
// The function creates a channel for the mempool transactions,
// and starts the mempool and forced_exit tasks.
pub fn run_forced_exit(connection_pool: ConnectionPool) -> Vec<JoinHandle<()>> {
    vlog::info!("Starting the ForcedExitRequests actors");
    let config = ForcedExitRequestsConfig::from_env();
    let common_config = CommonApiConfig::from_env();
    let contract_config = ContractsConfig::from_env();
    let eth_client_config = ETHClientConfig::from_env();
    let chain_config = ChainConfig::from_env();

    let (mempool_tx_request_sender, mempool_tx_request_receiver) =
        mpsc::channel(DEFAULT_CHANNEL_CAPACITY);
    let mempool_task = run_mempool_tx_handler(
        connection_pool.clone(),
        mempool_tx_request_receiver,
        chain_config.state_keeper.block_chunk_sizes,
    );
    let forced_exit_task = run_forced_exit_requests_actors(
        connection_pool,
        mempool_tx_request_sender,
        config,
        common_config,
        contract_config,
        eth_client_config.web3_url(),
    );
    vec![mempool_task, forced_exit_task]
}

// Run the Prover server actors.
// The function creates a new database instance
// and starts the Prover server task.
pub fn run_witness_generator(connection_pool: ConnectionPool) -> JoinHandle<()> {
    vlog::info!("Starting the Prover server actors");
    let prover_api_config = ProverApiConfig::from_env();
    let prover_config = ProverConfig::from_env();
    let database = zksync_witness_generator::database::Database::new(connection_pool);
    run_prover_server(database, prover_api_config, prover_config)
}

// Run the Ethereum sender actors.
// The function creates an instance of the EthereumGateway,
// and starts the Ethereum sender task.
pub fn run_eth_sender(connection_pool: ConnectionPool) -> JoinHandle<()> {
    vlog::info!("Starting the Ethereum sender actors");
    let eth_client_config = ETHClientConfig::from_env();
    let eth_sender_config = ETHSenderConfig::from_env();
    let contracts = ContractsConfig::from_env();
    let eth_gateway = EthereumGateway::from_config(
        &eth_client_config,
        &eth_sender_config,
        contracts.contract_addr,
    );

    zksync_eth_sender::run_eth_sender(connection_pool, eth_gateway, eth_sender_config)
}

// Run the price updater actors.
// The function creates a ticker instance and starts the price updater tasks.
pub fn run_price_updaters(connection_pool: ConnectionPool) -> Vec<JoinHandle<()>> {
    let ticker_config = TickerConfig::from_env();
    run_updaters(connection_pool, &ticker_config)
}

// Create an instance of the EthereumGateway.
// The function reads the required configuration from the environment
// and uses it to create the EthereumGateway instance.
pub fn create_eth_gateway() -> EthereumGateway {
    let eth_client_config = ETHClientConfig::from_env();
    let eth_sender_config = ETHSenderConfig::from_env();
    let contracts = ContractsConfig::from_env();
    EthereumGateway::from_config(
        &eth_client_config,
        &eth_sender_config,
        contracts.contract_addr,
    )
}
