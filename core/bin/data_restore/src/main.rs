pub mod contract_functions;
pub mod data_restore_driver;
pub mod eth_tx_helpers;
pub mod events;
pub mod events_state;
pub mod rollup_ops;
pub mod storage_interactor;
pub mod tree_state;

use crate::data_restore_driver::DataRestoreDriver;
use serde::Deserialize;
use structopt::StructOpt;
use web3::transports::Http;
use zksync_config::ConfigurationOptions;
use zksync_crypto::convert::FeConvert;
use zksync_storage::ConnectionPool;
use zksync_types::{
    tokens::{get_genesis_token_list, Token},
    Address, TokenId, H256,
};

// How many blocks we will process at once.
const ETH_BLOCKS_STEP: u64 = 10_000;
const END_ETH_BLOCKS_OFFSET: u64 = 40;

async fn add_tokens_to_db(pool: &ConnectionPool, eth_network: &str) {
    let genesis_tokens =
        get_genesis_token_list(&eth_network).expect("Initial token list not found");
    for (id, token) in (1..).zip(genesis_tokens) {
        log::info!(
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
                id: id as TokenId,
                symbol: token.symbol,
                address: token.address[2..]
                    .parse()
                    .expect("failed to parse token address"),
                decimals: token.decimals,
            })
            .await
            .expect("failed to store token");
    }
}

#[derive(StructOpt)]
#[structopt(
    name = "Data restore driver",
    author = "Matter Labs",
    rename_all = "snake_case"
)]
struct Opt {
    /// Restores data with provided genesis (zero) block
    #[structopt(long)]
    genesis: bool,

    /// Continues data restoring
    #[structopt(long = "continue", name = "continue")]
    continue_mode: bool,

    /// Restore data until the last verified block and exit
    #[structopt(long)]
    finite: bool,

    /// Expected tree root hash after restoring. This argument is ignored if mode is not `finite`
    #[structopt(long)]
    final_hash: Option<String>,

    /// Sets the web3 API to be used to interact with the Ethereum blockchain
    #[structopt(long = "web3", name = "web3")]
    web3_url: Option<String>,

    /// Provides a path to the configuration file for data restore
    #[structopt(long = "config", name = "config")]
    config_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ContractsConfig {
    eth_network: String,
    governance_addr: Address,
    genesis_tx_hash: H256,
    contract_addr: Address,
    available_block_chunk_sizes: Vec<usize>,
}

impl ContractsConfig {
    pub fn from_file(path: &str) -> Self {
        let content =
            std::fs::read_to_string(path).expect("Unable to find the specified config file");
        serde_json::from_str(&content).expect("Invalid configuration file provided")
    }

    pub fn from_env() -> Self {
        let config_opts = ConfigurationOptions::from_env();

        Self {
            eth_network: config_opts.eth_network,
            governance_addr: config_opts.governance_eth_addr,
            genesis_tx_hash: config_opts.genesis_tx_hash,
            contract_addr: config_opts.contract_eth_addr,
            available_block_chunk_sizes: config_opts.available_block_chunk_sizes,
        }
    }
}

#[tokio::main]
async fn main() {
    log::info!("Restoring zkSync state from the contract");
    env_logger::init();
    let connection_pool = ConnectionPool::new(Some(1)).await;
    let config_opts = ConfigurationOptions::from_env();

    let opt = Opt::from_args();

    let web3_url = opt.web3_url.unwrap_or(config_opts.web3_url);

    let transport = Http::new(&web3_url).expect("failed to start web3 transport");

    let config = opt
        .config_path
        .map(|path| ContractsConfig::from_file(&path))
        .unwrap_or_else(ContractsConfig::from_env);

    let finite_mode = opt.finite;
    let final_hash = if finite_mode {
        opt.final_hash
            .map(|value| FeConvert::from_hex(&value).expect("Can't parse the final hash"))
    } else {
        None
    };

    let mut driver = DataRestoreDriver::new(
        connection_pool,
        transport,
        config.governance_addr,
        config.contract_addr,
        ETH_BLOCKS_STEP,
        END_ETH_BLOCKS_OFFSET,
        config.available_block_chunk_sizes,
        finite_mode,
        final_hash,
    );

    // If genesis is argument is present - there will be fetching contracts creation transactions to get first eth block and genesis acc address
    if opt.genesis {
        // We have to load pre-defined tokens into the database before restoring state,
        // since these tokens do not have a corresponding Ethereum events.
        add_tokens_to_db(&driver.connection_pool, &config.eth_network).await;

        driver.set_genesis_state(config.genesis_tx_hash).await;
    }

    if opt.continue_mode {
        driver.load_state_from_storage().await;
    }

    driver.run_state_update().await;
}
