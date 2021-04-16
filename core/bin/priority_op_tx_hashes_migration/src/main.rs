use parity_crypto::digest::sha256;
use sqlx::{Connection, PgConnection};

async fn add_tx_hashes_to_existing_priority_ops() -> anyhow::Result<()> {
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let mut connection = PgConnection::connect(&database_url).await?;
    let mut transaction = connection.begin().await?;

    let ops = sqlx::query!(
        "SELECT priority_op_serialid, eth_hash, eth_block, eth_block_index FROM executed_priority_operations"
    )
    .fetch_all(&mut transaction)
    .await?;

    for op in ops {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&op.eth_hash);
        bytes.extend_from_slice(&(op.eth_block as u64).to_be_bytes());
        bytes.extend_from_slice(&(op.eth_block_index.unwrap_or(0) as u64).to_be_bytes());

        let tx_hash = sha256(&bytes);

        sqlx::query!(
            "UPDATE executed_priority_operations SET tx_hash = $1 WHERE priority_op_serialid = $2",
            &*tx_hash,
            op.priority_op_serialid
        )
        .execute(&mut transaction)
        .await?;
    }

    sqlx::query!("ALTER TABLE executed_priority_operations ALTER COLUMN tx_hash SET NOT NULL")
        .execute(&mut transaction)
        .await?;

    transaction.commit().await?;
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    add_tx_hashes_to_existing_priority_ops().await?;

    println!("Succesfully migrated");
    Ok(())
}
