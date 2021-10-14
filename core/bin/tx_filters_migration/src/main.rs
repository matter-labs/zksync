use zksync_storage::{utils::affected_accounts, StorageProcessor};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = StorageProcessor::establish_connection().await?;
    let mut transaction = storage.start_transaction().await?;

    let mut addresses = Vec::new();
    let mut tokens = Vec::new();
    let mut hashes = Vec::new();

    let txs = transaction
        .chain()
        .operations_ext_schema()
        .load_all_executed_transactions()
        .await?;
    for (hash, tx) in txs {
        let affected_accounts = affected_accounts(&tx, &mut transaction).await?;
        let used_tokens = tx.tokens();
        for address in affected_accounts {
            for token in used_tokens.clone() {
                addresses.push(address.as_bytes().to_vec());
                tokens.push(token.0 as i32);
                hashes.push(hash.as_bytes().to_vec());
            }
        }
    }

    let priority_ops = transaction
        .chain()
        .operations_ext_schema()
        .load_all_executed_priority_operations()
        .await?;
    for (hash, op) in priority_ops {
        let op = op.try_get_priority_op().unwrap();
        let affected_accounts = op.affected_accounts();
        let token = op.token_id();
        for address in affected_accounts {
            addresses.push(address.as_bytes().to_vec());
            tokens.push(token.0 as i32);
            hashes.push(hash.as_bytes().to_vec());
        }
    }

    transaction
        .chain()
        .operations_ext_schema()
        .save_executed_tx_filters(addresses, tokens, hashes)
        .await?;
    transaction.commit().await?;

    println!("Success");
    Ok(())
}
