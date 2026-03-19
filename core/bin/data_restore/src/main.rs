use structopt::StructOpt;
use web3::transports::Http;
use zksync_config::configs::ETHClientConfig;
use zksync_crypto::convert::FeConvert;
use zksync_storage::ConnectionPool;

use web3::Web3;
use zksync_data_restore::{
    add_tokens_to_storage, data_restore_driver::DataRestoreDriver,
    database_storage_interactor::DatabaseStorageInteractor, storage_interactor::StorageInteractor,
    END_ETH_BLOCKS_OFFSET, ETH_BLOCKS_STEP,
};
use zksync_l1_event_listener::{config::ContractsConfig, contract::ZkSyncDeployedContract};

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

#[tokio::main]
async fn main() {
    vlog::info!("Restoring zkSync state from the contract");
    let _vlog_guard = vlog::init();

    let opt = Opt::from_args();

    let web3_url = opt.web3_url.unwrap_or_else(|| {
        let config_opts = ETHClientConfig::from_env();
        config_opts.web3_url()
    });

    let config = opt
        .config_path
        .map(|path| ContractsConfig::from_file(&path))
        .unwrap_or_else(ContractsConfig::from_env);

    vlog::info!("Using the following config: {:#?}", config);

    let connection_pool = ConnectionPool::new(Some(1));
    let transport = Http::new(&web3_url).expect("failed to start web3 transport");
    let finite_mode = opt.finite;
    let final_hash = if finite_mode {
        opt.final_hash
            .map(|value| FeConvert::from_hex(&value).expect("Can't parse the final hash"))
    } else {
        None
    };
    let storage = connection_pool.access_storage().await.unwrap();
    let web3 = Web3::new(transport);
    let contract = ZkSyncDeployedContract::version4(web3.eth(), config.contract_addr);
    let mut driver = DataRestoreDriver::new(
        web3,
        config.governance_addr,
        config.upgrade_eth_blocks,
        config.init_contract_version,
        ETH_BLOCKS_STEP,
        END_ETH_BLOCKS_OFFSET,
        finite_mode,
        final_hash,
        contract,
    );

    let mut interactor = StorageInteractor::Database(DatabaseStorageInteractor::new(storage));
    // If genesis is argument is present - there will be fetching contracts creation transactions to get first eth block and genesis acc address
    if opt.genesis {
        // We have to load pre-defined tokens into the database before restoring state,
        // since these tokens do not have a corresponding Ethereum events.
        add_tokens_to_storage(&mut interactor, &config.eth_network.to_string()).await;

        driver
            .set_genesis_state_from_eth(&mut interactor, config.genesis_tx_hash)
            .await;
    }

    if opt.continue_mode && driver.load_state_from_storage(&mut interactor).await {
        std::process::exit(0);
    }

    driver.run_state_update(&mut interactor).await;
}
