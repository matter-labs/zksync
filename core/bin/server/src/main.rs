use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use std::str::FromStr;

use structopt::StructOpt;

use serde::{Deserialize, Serialize};

use zksync_api::fee_ticker::{run_ticker_task, TickerRequest};
use zksync_core::{genesis_init, run_core, wait_for_tasks};
use zksync_eth_client::EthereumGateway;
use zksync_forced_exit_requests::run_forced_exit_requests_actors;
use zksync_gateway_watcher::run_gateway_watcher_if_multiplexed;
use zksync_witness_generator::run_prover_server;

use tokio::task::JoinHandle;
use zksync_config::configs::api::PrometheusConfig;
use zksync_config::{
    configs::api::{
        CommonApiConfig, JsonRpcConfig, PrivateApiConfig, ProverApiConfig, RestApiConfig,
        Web3Config,
    },
    ChainConfig, ContractsConfig, DBConfig, ETHClientConfig, ETHSenderConfig, ETHWatchConfig,
    ForcedExitRequestsConfig, GatewayWatcherConfig, ProverConfig, TickerConfig, ZkSyncConfig,
};
use zksync_core::rejected_tx_cleaner::run_rejected_tx_cleaner;
use zksync_prometheus_exporter::run_prometheus_exporter;
use zksync_storage::ConnectionPool;

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
    EthSender,
    Core,
    WitnessGenerator,
    ForcedExit,

    // Additional components
    Prometheus,
    RejectedTaskCleaner,
}

impl FromStr for Component {
    type Err = String;

    fn from_str(s: &str) -> Result<Component, String> {
        match s {
            "rest-api" => Ok(Component::RestApi),
            "web3-api" => Ok(Component::Web3Api),
            "rpc-api" => Ok(Component::RpcApi),
            "rpc-websocket-api" => Ok(Component::RpcWebSocketApi),
            "eth-sender" => Ok(Component::EthSender),
            "witness-generator" => Ok(Component::WitnessGenerator),
            "forced-exit" => Ok(Component::ForcedExit),
            "prometheus" => Ok(Component::Prometheus),
            "core" => Ok(Component::Core),
            "rejected-task-cleaner" => Ok(Component::RejectedTaskCleaner),
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
        ])
    }
}

impl FromStr for ComponentsToRun {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
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
        default_value = "rest-api,web3-api,rpc-api,rpc-websocket-api,eth-sender,witness-generator,forced-exit,prometheus,core,rejected-task-cleaner"
    )]
    components: ComponentsToRun,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let mut _sentry_guard = None;
    let server_mode = if opt.genesis {
        ServerCommand::Genesis
    } else {
        _sentry_guard = vlog::init();
        ServerCommand::Launch
    };

    if let ServerCommand::Genesis = server_mode {
        vlog::info!("Performing the server genesis initialization",);
        let config = ChainConfig::from_env();
        genesis_init(&config).await;
        return Ok(());
    }

    // It's a `ServerCommand::Launch`, perform the usual routine.
    vlog::info!("Running the zkSync server");

    run_server(&opt.components).await;

    Ok(())
}

async fn run_server(components: &ComponentsToRun) {
    let connection_pool = ConnectionPool::new(None);
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    let channel_size = 32768;

    let mut tasks = vec![];

    if components.0.contains(&Component::Web3Api) {
        // Run web3 api
        tasks.push(zksync_api::api_server::web3::start_rpc_server(
            connection_pool.clone(),
            &Web3Config::from_env(),
        ));
    }

    if components.0.iter().any(|c| {
        matches!(
            c,
            Component::RpcWebSocketApi | Component::RpcApi | Component::RestApi
        )
    }) {
        // Create gateway
        let eth_gateway = create_eth_gateway();

        let eth_watch_config = ETHWatchConfig::from_env();
        let gateway_watcher_config = GatewayWatcherConfig::from_env();

        // Run eth multiplexer
        if let Some(task) =
            run_gateway_watcher_if_multiplexed(eth_gateway.clone(), &gateway_watcher_config)
        {
            tasks.push(task);
        }

        // Run ticker
        let (task, ticker_request_sender) = run_ticker(connection_pool.clone(), channel_size);
        tasks.push(task);

        // Run signer
        let (sign_check_sender, sign_check_receiver) = mpsc::channel(channel_size);
        tasks.push(zksync_api::signature_checker::start_sign_checker(
            eth_gateway,
            sign_check_receiver,
        ));

        let private_config = PrivateApiConfig::from_env();
        let contracts_config = ContractsConfig::from_env();
        let common_config = CommonApiConfig::from_env();
        let chain_config = ChainConfig::from_env();

        if components.0.contains(&Component::RpcWebSocketApi) {
            tasks.push(zksync_api::api_server::rpc_subscriptions::start_ws_server(
                connection_pool.clone(),
                sign_check_sender.clone(),
                ticker_request_sender.clone(),
                &common_config,
                &JsonRpcConfig::from_env(),
                chain_config.state_keeper.miniblock_iteration_interval(),
                private_config.url.clone(),
                eth_watch_config.confirmations_for_eth_event,
            ));
        }

        if components.0.contains(&Component::RpcApi) {
            tasks.push(zksync_api::api_server::rpc_server::start_rpc_server(
                connection_pool.clone(),
                sign_check_sender.clone(),
                ticker_request_sender.clone(),
                &JsonRpcConfig::from_env(),
                &common_config,
                private_config.url.clone(),
                eth_watch_config.confirmations_for_eth_event,
            ));
        }

        if components.0.contains(&Component::RestApi) {
            zksync_api::api_server::rest::start_server_thread_detached(
                connection_pool.clone(),
                RestApiConfig::from_env().bind_addr(),
                contracts_config.contract_addr,
                ticker_request_sender,
                sign_check_sender,
                private_config.url,
            );
        }
    }

    if components.0.contains(&Component::EthSender) {
        tasks.push(run_eth_sender(connection_pool.clone()))
    }

    if components.0.contains(&Component::Core) {
        let eth_gateway = create_eth_gateway();

        tasks.append(
            &mut run_core(
                connection_pool.clone(),
                &ZkSyncConfig::from_env(),
                eth_gateway.clone(),
            )
            .await
            .unwrap(),
        );
    }

    if components.0.contains(&Component::WitnessGenerator) {
        tasks.push(run_witness_generator(connection_pool.clone()))
    }

    if components.0.contains(&Component::Prometheus) {
        // Run prometheus data exporter.
        let config = PrometheusConfig::from_env();
        let (prometheus_task_handle, counter_task_handle) =
            run_prometheus_exporter(connection_pool.clone(), config.port, true);
        tasks.push(prometheus_task_handle);
        if let Some(task) = counter_task_handle {
            tasks.push(task);
        }
    }

    if components.0.contains(&Component::ForcedExit) {
        tasks.push(run_forced_exit(connection_pool.clone()));
    }

    if components.0.contains(&Component::RejectedTaskCleaner) {
        let config = DBConfig::from_env();
        tasks.push(run_rejected_tx_cleaner(&config, connection_pool));
    }

    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("Ctrl+C signal send");
        })
        .expect("Error setting Ctrl+C handler");
    }

    tokio::select! {
        _ = async { wait_for_tasks(tasks).await } => {
            panic!("One if the actors is not supposed to finish its execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            vlog::warn!("Stop signal received, shutting down");
        }
    };
}

pub fn run_forced_exit(connection_pool: ConnectionPool) -> JoinHandle<()> {
    vlog::info!("Starting the ForcedExitRequests actors");
    let config = ForcedExitRequestsConfig::from_env();
    let common_config = CommonApiConfig::from_env();
    let private_api_config = PrivateApiConfig::from_env();
    let contract_config = ContractsConfig::from_env();
    let eth_client_config = ETHClientConfig::from_env();

    run_forced_exit_requests_actors(
        connection_pool,
        private_api_config.url,
        config,
        common_config,
        contract_config,
        eth_client_config.web3_url(),
    )
}

pub fn run_witness_generator(connection_pool: ConnectionPool) -> JoinHandle<()> {
    vlog::info!("Starting the Prover server actors");
    let prover_api_config = ProverApiConfig::from_env();
    let prover_config = ProverConfig::from_env();
    let database = zksync_witness_generator::database::Database::new(connection_pool);
    run_prover_server(database, prover_api_config, prover_config)
}

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

pub fn run_ticker(
    connection_pool: ConnectionPool,
    channel_size: usize,
) -> (JoinHandle<()>, mpsc::Sender<TickerRequest>) {
    vlog::info!("Starting Ticker actors");
    let (ticker_request_sender, ticker_request_receiver) = mpsc::channel(channel_size);
    let chain_config = ChainConfig::from_env();
    let ticker_config = TickerConfig::from_env();
    let task = run_ticker_task(
        connection_pool,
        ticker_request_receiver,
        &ticker_config,
        chain_config.max_blocks_to_aggregate(),
    );
    (task, ticker_request_sender)
}

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
