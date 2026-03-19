use std::collections::HashMap;
use std::time::Duration;
use web3::{Transport, Web3, contract::Contract, ethabi, transports::Http, types::Address};
use zksync_config::ETHClientConfig;
use zksync_contracts::governance_contract;
use zksync_l1_event_listener::{
    config::ContractsConfig, eth_tx_helpers::get_ethereum_transaction, events_state::EventsState,
};
use zksync_types::{H256, TokenId};

use crate::{
    consts,
    consts::{ETH_BLOCKS_STEP, ETH_SYNC_CONFIRMATIONS, MAX_RETRIES, RESTORED_TOKENS_CSV},
    types::StorageToken,
};

pub fn run(web3_url: Option<String>, config_path: Option<String>) -> anyhow::Result<()> {
    let web3_url = web3_url.unwrap_or_else(|| {
        let config_opts = ETHClientConfig::from_env();
        config_opts.web3_url()
    });

    let config = config_path
        .map(|path| ContractsConfig::from_file(&path))
        .unwrap_or_else(ContractsConfig::from_env);

    println!(
        "Using the following config: {:#?} to restore token ids",
        config
    );

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    rt.block_on(restore_token_ids(&config, &web3_url))?;
    println!(
        "Token IDs restored successfully to {}",
        consts::RESTORED_TOKENS_CSV
    );
    Ok(())
}

/// Restores token IDs from ethereum events. It restores only fungible tokens added via the governance contract.
/// # Arguments
/// * `config` - Configuration containing contract addresses and genesis transaction hash
/// * `web3_url` - URL of the Ethereum JSON-RPC endpoint
async fn restore_token_ids(config: &ContractsConfig, web3_url: &str) -> anyhow::Result<()> {
    let transport = Http::new(web3_url).expect("failed to start web3 transport");
    let web3 = Web3::new(transport);
    let mut tokens = HashMap::new();
    tokens.insert(Address::zero(), TokenId(0)); // Insert ETH token
    let governance_contract = {
        let abi = governance_contract();
        (
            abi.clone(),
            Contract::new(web3.eth(), config.governance_addr, abi),
        )
    };

    let mut events_state = EventsState::default();

    set_genesis_state_from_eth(&web3, config.genesis_tx_hash, &mut events_state).await;
    println!(
        "Genesis block number set to {}",
        events_state.last_watched_eth_block_number
    );
    loop {
        for i in 0..=MAX_RETRIES {
            if i > 0 {
                println!("Retrying to load new data, attempt {}", i + 1);
            }
            match loop_iteration(&web3, &governance_contract, &mut events_state).await {
                Ok(Some(new_tokens)) => {
                    tokens.extend(new_tokens);
                    break;
                }
                Ok(None) => {
                    println!(
                        "Token ID restoration completed. Total tokens restored: {}",
                        tokens.len()
                    );
                    save_tokens_to_csv(tokens, RESTORED_TOKENS_CSV)?;
                    return Ok(());
                }
                Err(e) => {
                    println!("Failed to load new data from Ethereum: {}", e);
                    if i == MAX_RETRIES {
                        return Err(e);
                    } else {
                        println!("Error occurred: {}. Retrying...", e);
                        // wait a few seconds before the next attempt
                        tokio::time::sleep(Duration::from_secs(2)).await;
                    }
                }
            };
        }
    }
}

/// Do the iteration of getting tokens.
/// If none is returned that means the sync is finished
async fn loop_iteration<T: Transport>(
    web3: &Web3<T>,
    governance_contract: &(ethabi::Contract, Contract<T>),
    events_state: &mut EventsState,
) -> anyhow::Result<Option<HashMap<Address, TokenId>>> {
    let res = events_state
        .sync_is_finished(web3, ETH_SYNC_CONFIRMATIONS)
        .await?;
    if res {
        return Ok(None);
    }

    let (new_tokens, last_eth_processed_block) =
        load_new_data(web3, governance_contract, events_state).await?;
    events_state.last_watched_eth_block_number = last_eth_processed_block;
    Ok(Some(new_tokens))
}

/// Loads new token data from Ethereum events in a batch.
/// Processes events from the last watched block up to ETH_BLOCKS_STEP blocks ahead.
/// # Returns
/// Map of token addresses to their corresponding TokenIds
async fn load_new_data<T: Transport>(
    web3: &Web3<T>,
    governance_contract: &(ethabi::Contract, Contract<T>),
    events_state: &mut EventsState,
) -> anyhow::Result<(HashMap<Address, TokenId>, u64)> {
    let mut tokens = HashMap::new();
    let last_watched_eth_block_number = events_state.last_watched_eth_block_number;
    let from_block_number_u64 = last_watched_eth_block_number;
    let to_block_number_u64 = last_watched_eth_block_number + ETH_BLOCKS_STEP;
    let new_tokens = EventsState::get_token_added_logs(
        web3,
        governance_contract,
        from_block_number_u64.into(),
        to_block_number_u64.into(),
    )
    .await?;
    events_state.last_watched_eth_block_number =
        std::cmp::max(to_block_number_u64, events_state.latest_eth_block);

    for token in new_tokens {
        tokens.insert(token.address, token.id);
    }
    Ok((tokens, last_watched_eth_block_number))
}

/// Sets the genesis block from an Ethereum transaction.
async fn set_genesis_state_from_eth<T: Transport>(
    web3: &Web3<T>,
    genesis_tx_hash: H256,
    events_state: &mut EventsState,
) {
    let genesis_transaction = get_ethereum_transaction(web3, &genesis_tx_hash)
        .await
        .expect("Cant get zkSync genesis transaction");

    // Setting genesis block number for events state
    events_state
        .set_genesis_block_number(&genesis_transaction)
        .expect("Cant set genesis block number for events state");
}

/// Saves a map of tokens to a CSV file.
fn save_tokens_to_csv(tokens: HashMap<Address, TokenId>, path: &str) -> anyhow::Result<()> {
    let file = std::fs::File::create(path)?;
    let mut wtr = csv::Writer::from_writer(file);
    for token in tokens.into_iter() {
        wtr.serialize(StorageToken {
            address: token.0,
            id: token.1.0,
        })?
    }
    wtr.flush()?;
    Ok(())
}
