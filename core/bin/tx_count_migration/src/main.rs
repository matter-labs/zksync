use std::time::Duration;
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
                for i in 0..=10 {
                    let mut tr = transaction.start_transaction().await?;
                    match tr
                        .chain()
                        .operations_ext_schema()
                        .update_txs_count(start_account, final_account)
                        .await
                    {
                        Ok(_) => {
                            println!(
                                "Data for accounts from {:?} to {:?} has been updated",
                                &start_account, &final_account,
                            );
                            first_account = Some(final_account);
                            tr.commit().await?;
                            break;
                        }
                        Err(err) => {
                            let text = format!(
                                "Error for accounts from {:?} to {:?} detected {:?}",
                                &start_account, &final_account, err
                            );
                            if i != 10 {
                                println!("{}", text);
                            } else {
                                panic!("{}", text);
                            }
                        }
                    }

                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
            None => {
                // We can forget about tx because we will close
                // the connection without updating any data
                println!("Finish");
                break;
            }
        }
        tokio::time::sleep(Duration::from_secs(1)).await;
        transaction.commit().await?;
    }

    Ok(())
}
