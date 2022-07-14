use std::time::Duration;
use tokio::time::sleep;
use zksync_storage::StorageProcessor;

#[tokio::main]
async fn main() {
    let mut storage = StorageProcessor::establish_connection().await.unwrap();
    let mut last_seq_no_executed_txs = storage
        .chain()
        .operations_ext_schema()
        .get_last_seq_no()
        .await;
    println!("Last seq_no {}", last_seq_no_executed_txs);
    let mut last_seq_no_priority_ops = last_seq_no_executed_txs;
    let (_, mut max_seq_no) = storage
        .chain()
        .stats_schema()
        .count_total_transactions((last_seq_no_executed_txs as u64).into())
        .await
        .unwrap();
    let updated_tx_hashes = storage
        .chain()
        .operations_ext_schema()
        .update_non_unique_tx_filters_for_priority_ops()
        .await;

    println!("Finish updating non unique tx_filters");
    loop {
        let mut transaction = storage.start_transaction().await.unwrap();
        if last_seq_no_executed_txs < max_seq_no.0 as i64 {
            dbg!((
                last_seq_no_executed_txs,
                last_seq_no_priority_ops,
                max_seq_no
            ));
            last_seq_no_executed_txs = transaction
                .chain()
                .operations_ext_schema()
                .set_seq_no_for_executed_txs(last_seq_no_executed_txs)
                .await;
            println!("Last seq_no {}", last_seq_no_executed_txs);
            last_seq_no_priority_ops = transaction
                .chain()
                .operations_ext_schema()
                .set_unique_sequence_number_for_priority_operations(
                    last_seq_no_priority_ops,
                    &updated_tx_hashes,
                )
                .await;
            println!("Last seq_no priority {}", last_seq_no_priority_ops);
            transaction.commit().await.unwrap()
        } else {
            sleep(Duration::from_secs(10)).await;
            let (_, new_max_seq_no) = transaction
                .chain()
                .stats_schema()
                .count_total_transactions((last_seq_no_executed_txs as u64).into())
                .await
                .unwrap();
            max_seq_no = new_max_seq_no;
        }
    }
}
