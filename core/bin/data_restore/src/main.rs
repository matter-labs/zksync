#[macro_use]
extern crate log;

pub mod contract_functions;
pub mod data_restore_driver;
pub mod eth_tx_helpers;
pub mod events;
pub mod events_state;
pub mod rollup_ops;
pub mod storage_interactor;
pub mod tree_state;

use crate::data_restore_driver::DataRestoreDriver;
use clap::{App, Arg};
use web3::transports::Http;
use zksync_config::ConfigurationOptions;
use zksync_crypto::convert::fe_from_hex;
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

#[tokio::main]
async fn main() {
    info!("Restoring zkSync state from the contract");
    env_logger::init();
    let connection_pool = ConnectionPool::new(Some(1)).await;
    let config_opts = ConfigurationOptions::from_env();

    let cli = App::new("Data restore driver")
        .author("Matter Labs")
        .arg(
            Arg::with_name("genesis")
                .long("genesis")
                .help("Restores data with provided genesis (zero) block"),
        )
        .arg(
            Arg::with_name("continue")
                .long("continue")
                .help("Continues data restoring"),
        )
        .arg(
            Arg::with_name("finite")
                .long("finite")
                .help("Restore data until the last verified block and exit"),
        )
        .arg(
            Arg::with_name("final_hash")
                .long("final_hash")
                .takes_value(true)
                .help("Expected tree root hash after restoring. This argument is ignored if mode is not `finite`")
        )
        .get_matches();

    let transport = Http::new(&config_opts.web3_url).expect("failed to start web3 transport");
    let governance_addr = config_opts.governance_eth_addr;
    let genesis_tx_hash = config_opts.genesis_tx_hash;
    let contract_addr = config_opts.contract_eth_addr;
    let available_block_chunk_sizes = config_opts.available_block_chunk_sizes;

    let finite_mode = cli.is_present("finite");
    let final_hash = if finite_mode {
        cli.value_of("final_hash")
            .map(|value| fe_from_hex(value).expect("Can't parse the final hash"))
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
    if cli.is_present("genesis") {
        // We have to load pre-defined tokens into the database before restoring state,
        // since these tokens do not have a corresponding Ethereum events.
        add_tokens_to_db(&driver.connection_pool, &config_opts.eth_network).await;

        driver.set_genesis_state(genesis_tx_hash).await;
    }

    if cli.is_present("continue") {
        driver.load_state_from_storage().await;
    }

    driver.run_state_update().await;
}
