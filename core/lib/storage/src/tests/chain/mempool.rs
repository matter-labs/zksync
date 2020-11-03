// External imports
use zksync_crypto::rand::{Rng, SeedableRng, XorShiftRng};
// Workspace imports
use zksync_types::{
    mempool::SignedTxVariant,
    tx::{ChangePubKey, Transfer, Withdraw},
    Address, SignedZkSyncTx, ZkSyncTx,
};
// Local imports
use crate::tests::db_test;
use crate::{
    chain::{
        mempool::MempoolSchema,
        operations::{records::NewExecutedTransaction, OperationsSchema},
    },
    QueryResult, StorageProcessor,
};

use crate::tests::chain::utils::get_eth_sing_data;

/// Generates several different `SignedFranlinTx` objects.
fn franklin_txs() -> Vec<SignedZkSyncTx> {
    let transfer_1 = Transfer::new(
        42,
        Address::random(),
        Address::random(),
        0,
        100u32.into(),
        10u32.into(),
        10,
        None,
    );

    let transfer_2 = Transfer::new(
        4242,
        Address::random(),
        Address::random(),
        0,
        500u32.into(),
        20u32.into(),
        11,
        None,
    );

    let withdraw = Withdraw::new(
        33,
        Address::random(),
        Address::random(),
        0,
        100u32.into(),
        10u32.into(),
        12,
        None,
    );

    let change_pubkey = ChangePubKey::new(
        123,
        Address::random(),
        Default::default(),
        0,
        Default::default(),
        13,
        None,
        None,
    );

    let txs = [
        ZkSyncTx::Transfer(Box::new(transfer_1)),
        ZkSyncTx::Transfer(Box::new(transfer_2)),
        ZkSyncTx::Withdraw(Box::new(withdraw)),
        ZkSyncTx::ChangePubKey(Box::new(change_pubkey)),
    ];

    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    txs.iter()
        .map(|tx| {
            let test_message = format!("test message {}", rng.gen::<u32>());

            SignedZkSyncTx {
                tx: tx.clone(),
                eth_sign_data: Some(get_eth_sing_data(test_message)),
            }
        })
        .collect()
}

/// Generates the required number of transfer transactions.
fn gen_transfers(n: usize) -> Vec<SignedZkSyncTx> {
    let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

    (0..n)
        .map(|id| {
            let transfer = Transfer::new(
                id as u32,
                Address::random(),
                Address::random(),
                0,
                100u32.into(),
                10u32.into(),
                10,
                None,
            );

            let test_message = format!("test message {}", rng.gen::<u32>());

            SignedZkSyncTx {
                tx: ZkSyncTx::Transfer(Box::new(transfer)),
                eth_sign_data: Some(get_eth_sing_data(test_message)),
            }
        })
        .collect()
}

/// Gets a single transaction from a `SignedTxVariant`. Panics if variant is a batch.
fn unwrap_tx(tx: SignedTxVariant) -> SignedZkSyncTx {
    match tx {
        SignedTxVariant::Tx(tx) => tx,
        SignedTxVariant::Batch(_) => panic!("Attempt to unwrap a single transaction from a batch"),
    }
}

/// Checks the save&load routine for mempool schema.
#[db_test]
async fn store_load(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Insert several txs into the mempool schema.
    let txs = franklin_txs();
    for tx in &txs {
        MempoolSchema(&mut storage)
            .insert_tx(&tx.clone())
            .await
            .expect("Can't insert txs");
    }

    // Load the txs and check that they match the expected list.
    let txs_from_db = MempoolSchema(&mut storage)
        .load_txs()
        .await
        .expect("Can't load txs");
    assert_eq!(txs_from_db.len(), txs.len());

    for (tx, tx_from_db) in txs.iter().zip(txs_from_db) {
        let tx_from_db = unwrap_tx(tx_from_db);
        assert_eq!(tx_from_db.hash(), tx.hash(), "transaction changed");
        assert_eq!(
            tx_from_db.eth_sign_data, tx.eth_sign_data,
            "sign data changed"
        );
    }

    Ok(())
}

/// Checks the save&load routine for mempool schema.
#[db_test]
async fn store_load_batch(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Insert several txs into the mempool schema.
    let txs = gen_transfers(10);
    let alone_txs_1 = &txs[0..2];
    let batch_1 = &txs[2..4];
    let batch_2 = &txs[4..6];
    let alone_txs_2 = &txs[6..8];
    let batch_3 = &txs[8..10];

    let elements_count = alone_txs_1.len() + alone_txs_2.len() + 3; // Amount of alone txs + amount of batches.

    for tx in alone_txs_1 {
        MempoolSchema(&mut storage).insert_tx(tx).await?;
    }

    MempoolSchema(&mut storage).insert_batch(batch_1).await?;

    MempoolSchema(&mut storage).insert_batch(batch_2).await?;

    for tx in alone_txs_2 {
        MempoolSchema(&mut storage).insert_tx(tx).await?;
    }

    MempoolSchema(&mut storage).insert_batch(batch_3).await?;

    // Load the txs and check that they match the expected list.
    let txs_from_db = MempoolSchema(&mut storage).load_txs().await?;
    assert_eq!(txs_from_db.len(), elements_count);

    assert!(matches!(txs_from_db[0], SignedTxVariant::Tx(_)));
    assert!(matches!(txs_from_db[1], SignedTxVariant::Tx(_)));
    assert!(matches!(txs_from_db[2], SignedTxVariant::Batch(_)));
    assert!(matches!(txs_from_db[3], SignedTxVariant::Batch(_)));
    assert!(matches!(txs_from_db[4], SignedTxVariant::Tx(_)));
    assert!(matches!(txs_from_db[5], SignedTxVariant::Tx(_)));
    assert!(matches!(txs_from_db[6], SignedTxVariant::Batch(_)));

    Ok(())
}

/// Checks that removed txs won't appear on the next load.
#[db_test]
async fn remove_txs(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Point at which txs will be split into removed / retained.
    const SPLIT_TXS_AT: usize = 2;

    // Insert several txs into the mempool schema.
    let txs = franklin_txs();
    for tx in &txs {
        MempoolSchema(&mut storage).insert_tx(&tx.clone()).await?;
    }

    // Remove several txs from the schema.
    let hashes_to_remove: Vec<_> = txs[SPLIT_TXS_AT..]
        .iter()
        .map(|tx| tx.hash().as_ref().to_vec())
        .collect();
    let retained_hashes: Vec<_> = txs[..SPLIT_TXS_AT].iter().map(|tx| tx.hash()).collect();
    for hash in hashes_to_remove {
        MempoolSchema(&mut storage).remove_tx(&hash).await?;
    }

    // Load the txs and check that they match the expected list.
    let txs_from_db = MempoolSchema(&mut storage).load_txs().await?;
    assert_eq!(txs_from_db.len(), retained_hashes.len());

    for (expected_hash, tx_from_db) in retained_hashes.iter().zip(txs_from_db) {
        assert_eq!(*expected_hash, unwrap_tx(tx_from_db).hash());
    }

    Ok(())
}

/// Checks that already committed txs are removed by `collect_garbage` method.
#[db_test]
async fn collect_garbage(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    // Insert several txs into the mempool schema.
    let txs = franklin_txs();
    for tx in &txs {
        MempoolSchema(&mut storage)
            .insert_tx(&tx.clone())
            .await
            .expect("Can't insert txs");
    }

    // Add one executed transaction.
    let executed_tx = NewExecutedTransaction {
        block_number: 1,
        tx_hash: txs[0].hash().as_ref().to_vec(),
        tx: Default::default(),
        operation: Default::default(),
        from_account: Default::default(),
        to_account: None,
        success: true,
        fail_reason: None,
        block_index: None,
        primary_account_address: Default::default(),
        nonce: Default::default(),
        created_at: chrono::Utc::now(),
        eth_sign_data: None,
        batch_id: None,
    };
    OperationsSchema(&mut storage)
        .store_executed_tx(executed_tx)
        .await?;

    // Collect the garbage. Execution transaction (very first one from the list)
    // should be removed from the schema.
    MempoolSchema(&mut storage).collect_garbage().await?;
    let retained_hashes: Vec<_> = txs[1..].iter().map(|tx| tx.hash()).collect();

    // Load the txs and check that they match the expected list.
    let txs_from_db = MempoolSchema(&mut storage).load_txs().await?;
    assert_eq!(txs_from_db.len(), retained_hashes.len());

    for (expected_hash, tx_from_db) in retained_hashes.iter().zip(txs_from_db) {
        assert_eq!(*expected_hash, unwrap_tx(tx_from_db).hash());
    }

    Ok(())
}
