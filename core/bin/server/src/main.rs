use futures::{channel::mpsc, StreamExt};

use structopt::StructOpt;

use serde::{Deserialize, Serialize};

use zksync_core::{genesis_init, run_core, wait_for_tasks};
use zksync_eth_client::EthereumGateway;
use zksync_eth_sender::run_eth_sender;
use zksync_forced_exit_requests::run_forced_exit_requests_actors;
use zksync_gateway_watcher::run_gateway_watcher_if_multiplexed;

use zksync_witness_generator::run_prover_server;

use std::str::FromStr;
use zksync_api::fee_ticker::run_ticker_task;
use zksync_config::configs::api::{
    CommonApiConfig, JsonRpcConfig, PrivateApiConfig, ProverApiConfig, RestApiConfig, Web3Config,
};
use zksync_config::{
    ChainConfig, ContractsConfig, DBConfig, ETHClientConfig, ETHSenderConfig, ETHWatchConfig,
    ForcedExitRequestsConfig, GatewayWatcherConfig, ProverConfig, TickerConfig, ZkSyncConfig,
};
use zksync_core::rejected_tx_cleaner::run_rejected_tx_cleaner;
use zksync_storage::ConnectionPool;

#[derive(Debug, Clone, Copy)]
pub enum ServerCommand {
    Genesis,
    Launch,
}

#[derive(Serialize, Deserialize, Debug, Eq, PartialEq)]
pub enum Component {
    RestApi,
    Web3Api,
    RpcApi,
    RpcWebSocketApi,

    EthSender,

    WitnessGenerator,
    ForcedExit,
    Prometheus,

    Core,
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

impl FromStr for ComponentsToRun {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(ComponentsToRun(
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
    println!("{:?}", components);
    let connection_pool = ConnectionPool::new(None);
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);

    let mut tasks = vec![];

    if components.0.contains(&Component::Web3Api) {
        let config = Web3Config::from_env();
        zksync_api::api_server::web3::start_rpc_server(
            connection_pool.clone(),
            stop_signal_sender.clone(),
            &config,
        );
    }

    if components.0.iter().any(|c| {
        matches!(
            c,
            Component::RpcWebSocketApi
                | Component::RpcApi
                | Component::RestApi
                | Component::EthSender
        )
    }) {
        let eth_client_config = ETHClientConfig::from_env();
        let eth_sender_config = ETHSenderConfig::from_env();
        let eth_watch_config = ETHWatchConfig::from_env();
        let contracts = ContractsConfig::from_env();
        let eth_gateway = EthereumGateway::from_config(
            &eth_client_config,
            &eth_sender_config,
            contracts.contract_addr,
        );

        let gateway_watcher_config = GatewayWatcherConfig::from_env();
        let _gateway_watcher_task_opt =
            run_gateway_watcher_if_multiplexed(eth_gateway.clone(), &gateway_watcher_config);
        let channel_size = 32768;

        let (ticker_request_sender, ticker_request_receiver) = mpsc::channel(channel_size);
        let chain_config = ChainConfig::from_env();

        let max_blocks_to_aggregate = std::cmp::max(
            chain_config.state_keeper.max_aggregated_blocks_to_commit,
            chain_config.state_keeper.max_aggregated_blocks_to_execute,
        ) as u32;
        let ticker_config = TickerConfig::from_env();

        let ticker_task = run_ticker_task(
            connection_pool.clone(),
            ticker_request_receiver,
            &ticker_config,
            max_blocks_to_aggregate,
        );

        tasks.push(ticker_task);
        let (sign_check_sender, sign_check_receiver) = mpsc::channel(32768);

        zksync_api::signature_checker::start_sign_checker_detached(
            eth_gateway.clone(),
            sign_check_receiver,
            stop_signal_sender.clone(),
        );

        let private_config = PrivateApiConfig::from_env();

        let all_config = ZkSyncConfig::from_env();
        tasks.append(
            &mut run_core(
                connection_pool.clone(),
                &all_config,
                stop_signal_sender.clone(),
                eth_gateway.clone(),
            )
            .await
            .unwrap(),
        );

        let contracts_config = ContractsConfig::from_env();
        let common_config = CommonApiConfig::from_env();

        if components.0.contains(&Component::RpcWebSocketApi) {
            let config = JsonRpcConfig::from_env();
            zksync_api::api_server::rpc_subscriptions::start_ws_server(
                connection_pool.clone(),
                sign_check_sender.clone(),
                ticker_request_sender.clone(),
                stop_signal_sender.clone(),
                &common_config,
                &config,
                chain_config.state_keeper.miniblock_iteration_interval(),
                private_config.url.clone(),
                eth_watch_config.confirmations_for_eth_event,
            );
        }

        if components.0.contains(&Component::RpcApi) {
            let config = JsonRpcConfig::from_env();
            zksync_api::api_server::rpc_server::start_rpc_server(
                connection_pool.clone(),
                sign_check_sender.clone(),
                ticker_request_sender.clone(),
                stop_signal_sender.clone(),
                &config,
                &common_config,
                private_config.url.clone(),
                eth_watch_config.confirmations_for_eth_event,
            );
        }

        if components.0.contains(&Component::RestApi) {
            let config = RestApiConfig::from_env();
            zksync_api::api_server::rest::start_server_thread_detached(
                connection_pool.clone(),
                config.bind_addr(),
                contracts_config.contract_addr,
                stop_signal_sender.clone(),
                ticker_request_sender,
                sign_check_sender,
                private_config.url,
            );
        }
        if components.0.contains(&Component::EthSender) {
            // Run Ethereum sender actors.
            vlog::info!("Starting the Ethereum sender actors");
            let config = ETHSenderConfig::from_env();
            tasks.push(run_eth_sender(connection_pool.clone(), eth_gateway, config));
        }
    }

    if components.0.contains(&Component::WitnessGenerator) {
        vlog::info!("Starting the Prover server actors");
        let prover_api_config = ProverApiConfig::from_env();
        let prover_config = ProverConfig::from_env();
        let database = zksync_witness_generator::database::Database::new(connection_pool.clone());
        run_prover_server(
            database,
            stop_signal_sender,
            prover_api_config,
            prover_config,
        );
    }

    if components.0.contains(&Component::ForcedExit) {
        vlog::info!("Starting the ForcedExitRequests actors");
        let config = ForcedExitRequestsConfig::from_env();
        let common_config = CommonApiConfig::from_env();
        let private_api_config = PrivateApiConfig::from_env();
        let contract_config = ContractsConfig::from_env();
        let eth_client_config = ETHClientConfig::from_env();

        tasks.push(run_forced_exit_requests_actors(
            connection_pool.clone(),
            private_api_config.url,
            config,
            common_config,
            contract_config,
            eth_client_config.web3_url(),
        ))
    }

    if components.0.contains(&Component::RejectedTaskCleaner) {
        let config = DBConfig::from_env();
        tasks.push(run_rejected_tx_cleaner(&config, connection_pool));
    }

    tokio::select! {
        _ = async { wait_for_tasks(tasks).await } => {
            panic!("ForcedExitRequests actor is not supposed to finish its execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            vlog::warn!("Stop signal received, shutting down");
        }
    };
}
