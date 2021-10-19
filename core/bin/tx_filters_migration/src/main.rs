use tokio::time::{sleep, Duration};
use zksync_storage::{utils::affected_accounts, StorageProcessor};

const DELTA: u32 = 100;
const TIMEOUT: Duration = Duration::from_secs(2);

macro_rules! wait_for_success {
    ($l:expr) => {
        loop {
            match $l {
                Ok(result) => break result,
                Err(e) => println!("{}", e),
            }
            sleep(TIMEOUT).await;
        }
    };
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut storage = StorageProcessor::establish_connection().await?;

    let mut last_updated_block = wait_for_success!(
        storage
            .chain()
            .operations_ext_schema()
            .last_block_with_updated_tx_filters()
            .await
    );

    loop {
        sleep(TIMEOUT).await;
        let mut transaction = wait_for_success!(storage.start_transaction().await);
        let last_saved_block = wait_for_success!(
            transaction
                .chain()
                .block_schema()
                .get_last_saved_block()
                .await
        );

        let from = last_updated_block;
        let to = std::cmp::min(last_updated_block + DELTA, last_saved_block);

        let mut addresses = Vec::new();
        let mut tokens = Vec::new();
        let mut hashes = Vec::new();

        let txs = wait_for_success!(
            transaction
                .chain()
                .operations_ext_schema()
                .load_executed_txs_in_block_range(from, to)
                .await
        );
        for (hash, tx) in txs {
            let affected_accounts =
                wait_for_success!(affected_accounts(&tx, &mut transaction).await);
            let used_tokens = tx.tokens();
            for address in affected_accounts {
                for token in used_tokens.clone() {
                    addresses.push(address.as_bytes().to_vec());
                    tokens.push(token.0 as i32);
                    hashes.push(hash.as_bytes().to_vec());
                }
            }
        }

        let priority_ops = wait_for_success!(
            transaction
                .chain()
                .operations_ext_schema()
                .load_executed_priority_ops_in_block_range(from, to)
                .await
        );
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

        wait_for_success!(
            transaction
                .chain()
                .operations_ext_schema()
                .save_executed_tx_filters(addresses.clone(), tokens.clone(), hashes.clone())
                .await
        );

        match transaction.commit().await {
            Ok(_) => {
                println!("Updated from {} to {}", from, to);
                last_updated_block = to;
            }
            Err(e) => {
                println!("Failed to commit transaction: {}", e);
            }
        }
    }
}
