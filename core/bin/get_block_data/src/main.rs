use anyhow::{ensure, format_err};
use ethabi::Token;
use sqlx::{Connection, PgConnection, Postgres, Transaction};
use std::env;
use web3::{
    contract::Options,
    types::{TransactionReceipt, U256, U64},
};
use zksync_config::ZkSyncConfig;
use zksync_eth_client::EthereumGateway;
use zksync_storage::ConnectionPool;
use zksync_types::{aggregated_operations::stored_block_info, BlockNumber};

async fn send_raw_tx_and_wait(
    client: &EthereumGateway,
    raw_tx: Vec<u8>,
) -> Result<TransactionReceipt, anyhow::Error> {
    let tx_hash = client
        .send_raw_tx(raw_tx)
        .await
        .map_err(|e| format_err!("Failed to send raw tx: {}", e))?;
    loop {
        if let Some(receipt) = client
            .tx_receipt(tx_hash)
            .await
            .map_err(|e| format_err!("Failed to get receipt from eth node: {}", e))?
        {
            return Ok(receipt);
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = env::args().collect();
    ensure!(
        args.len() == 2,
        "There should be exactly one argument - count of blocks to revert"
    );
    let blocks_to_revert: u32 = args[1].parse()?;

    let connection_pool = ConnectionPool::new(Some(1));
    let mut storage = connection_pool.access_storage().await?;

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

    let mut blocks = Vec::new();
    for block_number in ((*last_commited_block - blocks_to_revert + 1)..=*last_commited_block).rev()
    {
        println!("{}", block_number);
        let block = storage
            .chain()
            .block_schema()
            .get_block(BlockNumber(block_number))
            .await?
            .unwrap();
        blocks.push(block);
    }

    let config = ZkSyncConfig::from_env();
    let client = EthereumGateway::from_config(&config);

    let tx_arg = Token::Array(blocks.iter().map(stored_block_info).collect());
    let data = client.encode_tx_data("revertBlocks", tx_arg);
    let signed_tx = client
        .sign_prepared_tx(
            data,
            Options::with(|f| f.gas = Some(U256::from(9 * 10u64.pow(6)))),
        )
        .await
        .map_err(|e| format_err!("Revert blocks send err: {}", e))?;
    let receipt = send_raw_tx_and_wait(&client, signed_tx.raw_tx).await?;
    ensure!(receipt.status == Some(U64::from(1)), "Tx failed");

    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut connection: PgConnection = PgConnection::connect(&database_url).await?;
    let mut transaction: Transaction<'_, Postgres> = connection.begin().await?;
    let last_reverted_block = (*last_commited_block - blocks_to_revert + 1) as i64;

    sqlx::query!("DELETE FROM blocks WHERE number >= $1", last_reverted_block)
        .execute(&mut transaction)
        .await?;
    sqlx::query!(
        "DELETE FROM account_balance_updates WHERE block_number >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM account_creates WHERE block_number >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM account_pubkey_updates WHERE block_number >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM block_witness WHERE block >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM proofs WHERE block_number >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM aggregated_proofs WHERE last_block >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!("DELETE FROM pending_block")
        .execute(&mut transaction)
        .await?;
    sqlx::query!(
        "DELETE FROM account_tree_cache WHERE block >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "DELETE FROM executed_priority_operations WHERE block_number >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;

    sqlx::query!(
        "DELETE FROM prover_job_queue WHERE first_block >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "UPDATE prover_job_queue SET last_block = $1 WHERE last_block >= $2",
        last_reverted_block - 1,
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;

    let op_ids = sqlx::query!(
        "SELECT id FROM aggregate_operations WHERE from_block >= $1",
        last_reverted_block
    )
    .fetch_all(&mut transaction)
    .await?;
    for op_record in op_ids {
        let eth_op_ids = sqlx::query!(
            "SELECT eth_op_id FROM eth_aggregated_ops_binding WHERE op_id = $1",
            op_record.id
        )
        .fetch_all(&mut transaction)
        .await?;
        sqlx::query!(
            "DELETE FROM eth_aggregated_ops_binding WHERE op_id = $1",
            op_record.id
        )
        .execute(&mut transaction)
        .await?;
        for eth_op_record in eth_op_ids {
            sqlx::query!(
                "DELETE FROM eth_tx_hashes WHERE eth_op_id = $1",
                eth_op_record.eth_op_id
            )
            .execute(&mut transaction)
            .await?;
            sqlx::query!(
                "DELETE FROM eth_operations WHERE id = $1",
                eth_op_record.eth_op_id
            )
            .execute(&mut transaction)
            .await?;
        }
    }
    sqlx::query!(
        "DELETE FROM aggregate_operations WHERE from_block >= $1",
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "UPDATE aggregate_operations SET to_block = $1 WHERE to_block >= $2",
        last_reverted_block - 1,
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;

    sqlx::query!(
        "UPDATE eth_parameters SET nonce = $1 WHERE id = true",
        client.current_nonce().await?.as_u64() as i64
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        "UPDATE eth_parameters SET last_committed_block = $1 WHERE id = true",
        last_reverted_block - 1
    )
    .execute(&mut transaction)
    .await?;

    sqlx::query!(
        r#"
        INSERT INTO mempool_txs (tx_hash, tx, created_at, eth_sign_data, batch_id)
        SELECT tx_hash, tx, created_at, eth_sign_data, batch_id FROM executed_transactions
        WHERE block_number >= $1
    "#,
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;
    sqlx::query!(
        r#"
        DELETE FROM executed_transactions
        WHERE block_number >= $1
    "#,
        last_reverted_block
    )
    .execute(&mut transaction)
    .await?;

    transaction.commit().await?;
    Ok(())
}
