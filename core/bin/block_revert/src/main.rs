use anyhow::{bail, ensure, format_err};
use ethabi::Token;
use structopt::StructOpt;
use tokio::time::Duration;
use web3::{
    contract::Options,
    types::{TransactionReceipt, U256, U64},
};
use zksync_config::ZkSyncConfig;
use zksync_eth_client::EthereumGateway;
use zksync_storage::StorageProcessor;
use zksync_types::{aggregated_operations::stored_block_info, block::Block, BlockNumber, Nonce};

async fn revert_blocks_in_storage(
    client: &EthereumGateway,
    storage: &mut StorageProcessor<'_>,
    last_block: BlockNumber,
) -> anyhow::Result<()> {
    let mut transaction = storage.start_transaction().await?;

    transaction
        .chain()
        .block_schema()
        .remove_blocks(last_block)
        .await?;
    transaction
        .chain()
        .block_schema()
        .remove_pending_block()
        .await?;
    transaction
        .chain()
        .block_schema()
        .remove_account_tree_cache(last_block)
        .await?;

    transaction
        .chain()
        .state_schema()
        .remove_account_balance_updates(last_block)
        .await?;
    transaction
        .chain()
        .state_schema()
        .remove_account_creates(last_block)
        .await?;
    transaction
        .chain()
        .state_schema()
        .remove_account_pubkey_updates(last_block)
        .await?;

    transaction
        .chain()
        .operations_schema()
        .remove_executed_priority_operations(last_block)
        .await?;
    transaction
        .chain()
        .operations_schema()
        .remove_aggregate_operations_and_bindings(last_block)
        .await?;

    transaction
        .prover_schema()
        .remove_witnesses(last_block)
        .await?;
    transaction
        .prover_schema()
        .remove_proofs(last_block)
        .await?;
    transaction
        .prover_schema()
        .remove_aggregated_proofs(last_block)
        .await?;
    transaction
        .prover_schema()
        .remove_prover_jobs(last_block)
        .await?;

    // Nonce after reverting on the contract will be current plus one
    // because the operator will send exactly one transaction to call revertBlocks.
    let nonce_after_revert_on_contract = client.current_nonce().await?.as_u32() + 1;

    transaction
        .ethereum_schema()
        .update_eth_parameters(last_block, Nonce(nonce_after_revert_on_contract))
        .await?;

    transaction
        .chain()
        .mempool_schema()
        .return_executed_txs_to_mempool(last_block)
        .await?;

    transaction.commit().await?;
    Ok(())
}

async fn send_raw_tx_and_wait_confirmation(
    client: &EthereumGateway,
    raw_tx: Vec<u8>,
) -> Result<TransactionReceipt, anyhow::Error> {
    let tx_hash = client
        .send_raw_tx(raw_tx)
        .await
        .map_err(|e| format_err!("Failed to send raw tx: {}", e))?;

    let mut poller = tokio::time::interval(Duration::from_millis(100));
    let start = std::time::Instant::now();
    let confirmation_timeout = Duration::from_secs(10);

    loop {
        if let Some(receipt) = client
            .tx_receipt(tx_hash)
            .await
            .map_err(|e| format_err!("Failed to get receipt from eth node: {}", e))?
        {
            return Ok(receipt);
        }

        if start.elapsed() > confirmation_timeout {
            bail!("Operation timeout");
        }
        poller.tick().await;
    }
}

async fn revert_blocks_on_contract(
    client: &EthereumGateway,
    blocks: &[Block],
    gas_limit: u32,
) -> anyhow::Result<()> {
    let tx_arg = Token::Array(blocks.iter().map(stored_block_info).collect());
    let data = client.encode_tx_data("revertBlocks", tx_arg);
    let signed_tx = client
        .sign_prepared_tx(data, Options::with(|f| f.gas = Some(U256::from(gas_limit))))
        .await
        .map_err(|e| format_err!("Revert blocks send err: {}", e))?;
    let receipt = send_raw_tx_and_wait_confirmation(&client, signed_tx.raw_tx).await?;
    ensure!(
        receipt.status == Some(U64::from(1)),
        "Tx to contract failed"
    );

    Ok(())
}

async fn get_blocks(
    last_commited_block: BlockNumber,
    blocks_to_revert: u32,
    storage: &mut StorageProcessor<'_>,
) -> Result<Vec<Block>, anyhow::Error> {
    let mut blocks = Vec::new();
    let last_block_to_revert = *last_commited_block - blocks_to_revert + 1;
    let range_to_revert = last_block_to_revert..=*last_commited_block;
    for block_number in range_to_revert.rev() {
        let block = storage
            .chain()
            .block_schema()
            .get_block(BlockNumber(block_number))
            .await?
            .unwrap();
        blocks.push(block);
    }
    Ok(blocks)
}

#[derive(Debug, StructOpt)]
struct Opt {
    #[structopt(long)]
    number: u32,
    #[structopt(long)]
    gas_limit: u32,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let opt = Opt::from_args();
    let blocks_to_revert = opt.number;

    let mut storage = StorageProcessor::establish_connection().await?;
    let config = ZkSyncConfig::from_env();
    let client = EthereumGateway::from_config(&config);

    let last_commited_block = storage
        .chain()
        .block_schema()
        .get_last_committed_block()
        .await?;
    let last_verified_block = storage
        .chain()
        .block_schema()
        .get_last_verified_confirmed_block()
        .await?;

    ensure!(
        last_verified_block + blocks_to_revert <= last_commited_block,
        "Some blocks to revert are already verified"
    );

    let blocks = get_blocks(last_commited_block, blocks_to_revert, &mut storage).await?;
    let last_block = BlockNumber(*last_commited_block - blocks_to_revert);
    let mut transaction = storage.start_transaction().await?;

    // 1. Try to revert blocks in storage, if it fails return error.
    // 2. Try to revert blocks on contract, if it fails changes in storage will revert.
    revert_blocks_in_storage(&client, &mut transaction, last_block).await?;
    revert_blocks_on_contract(&client, &blocks, opt.gas_limit).await?;

    transaction.commit().await?;
    Ok(())
}
