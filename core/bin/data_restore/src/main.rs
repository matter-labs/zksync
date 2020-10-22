pub mod contract_functions;
pub mod data_restore_driver;
pub mod eth_tx_helpers;
pub mod events;
pub mod events_state;
pub mod rollup_ops;
pub mod storage_interactor;
pub mod tree_state;

use crate::data_restore_driver::DataRestoreDriver;
use structopt::StructOpt;
use web3::transports::Http;
use zksync_config::ConfigurationOptions;
use zksync_crypto::convert::FeConvert;
use zksync_storage::ConnectionPool;
use zksync_types::{
    tokens::{get_genesis_token_list, Token},
    TokenId,
};

const ETH_BLOCKS_STEP: u64 = 1;
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
    #[structopt(long, name = "continue")]
    continue_mode: bool,

    /// Restore data until the last verified block and exit
    #[structopt(long)]
    finite: bool,

    /// Expected tree root hash after restoring. This argument is ignored if mode is not `finite`
    #[structopt(long)]
    final_hash: Option<String>,
}

#[tokio::main]
async fn main() {
    log::info!("Restoring zkSync state from the contract");
    env_logger::init();
    let connection_pool = ConnectionPool::new(Some(1)).await;
    let config_opts = ConfigurationOptions::from_env();

    let opt = Opt::from_args();

    let transport = Http::new(&config_opts.web3_url).expect("failed to start web3 transport");
    let governance_addr = config_opts.governance_eth_addr;
    let genesis_tx_hash = config_opts.genesis_tx_hash;
    let contract_addr = config_opts.contract_eth_addr;
    let available_block_chunk_sizes = config_opts.available_block_chunk_sizes;

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
        governance_addr,
        contract_addr,
        ETH_BLOCKS_STEP,
        END_ETH_BLOCKS_OFFSET,
        available_block_chunk_sizes,
        finite_mode,
        final_hash,
    );

    // If genesis is argument is present - there will be fetching contracts creation transactions to get first eth block and genesis acc address
    if opt.genesis {
        // We have to load pre-defined tokens into the database before restoring state,
        // since these tokens do not have a corresponding Ethereum events.
        add_tokens_to_db(&driver.connection_pool, &config_opts.eth_network).await;

        driver.set_genesis_state(genesis_tx_hash).await;
    }

    if opt.continue_mode {
        driver.load_state_from_storage().await;
    }

    driver.run_state_update().await;
}
