use zksync_storage::StorageProcessor;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = StorageProcessor::establish_connection().await?;
    let mut first_account = None;
    loop {
        let mut transaction = storage.start_transaction().await?;
        match transaction
            .chain()
            .operations_ext_schema()
            .get_accounts_range(first_account, 10000)
            .await
        {
            Some((start_account, final_account)) => {
                transaction
                    .chain()
                    .operations_ext_schema()
                    .update_txs_count(start_account, final_account)
                    .await;
                println!(
                    "Data for accounts from {:?} to {:?} has been updated",
                    &start_account, &final_account,
                );
                first_account = Some(final_account);
            }
            None => {
                // We can forget about tx because we will close
                // the connection without updating any data
                println!("Finish");
                break;
            }
        }
        transaction.commit().await?;
    }

    Ok(())
}
