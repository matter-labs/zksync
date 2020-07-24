// External imports
// Workspace imports
use models::node::{
    mempool::TxVariant,
    tx::{ChangePubKey, Transfer, Withdraw},
    Address, FranklinTx,
};
// Local imports
use crate::tests::db_test;
use crate::{
    chain::{
        mempool::MempoolSchema,
        operations::{records::NewExecutedTransaction, OperationsSchema},
    },
    StorageProcessor,
};

/// Generates several different `FranlinTx` objects.
fn franklin_txs() -> Vec<FranklinTx> {
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

    let change_pubkey = ChangePubKey {
        account_id: 123,
        account: Address::random(),
        new_pk_hash: Default::default(),
        nonce: 13,
        eth_signature: None,
    };

    vec![
        FranklinTx::Transfer(Box::new(transfer_1)),
        FranklinTx::Transfer(Box::new(transfer_2)),
        FranklinTx::Withdraw(Box::new(withdraw)),
        FranklinTx::ChangePubKey(Box::new(change_pubkey)),
    ]
}

/// Generates the required number of transfer transactions.
fn gen_transfers(n: usize) -> Vec<FranklinTx> {
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

            FranklinTx::Transfer(Box::new(transfer))
        })
        .collect()
}

/// Gets a single transaction from a `TxVariant`. Panics if variant is a batch.
fn unwrap_tx(tx: TxVariant) -> FranklinTx {
    match tx {
        TxVariant::Tx(tx) => tx,
        TxVariant::Batch(_) => panic!("Attempt to unwrap a single transaction from a batch"),
    }
}

/// Checks the save&load routine for mempool schema.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn store_load() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Insert several txs into the mempool schema.
        let txs = franklin_txs();
        for tx in &txs {
            MempoolSchema(&conn)
                .insert_tx(tx)
                .expect("Can't insert txs");
        }

        // Load the txs and check that they match the expected list.
        let txs_from_db = MempoolSchema(&conn).load_txs().expect("Can't load txs");
        assert_eq!(txs_from_db.len(), txs.len());

        for (tx, tx_from_db) in txs.iter().zip(txs_from_db) {
            assert_eq!(unwrap_tx(tx_from_db).hash(), tx.hash());
        }

        Ok(())
    });
}

/// Checks the save&load routine for mempool schema.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn store_load_batch() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Insert several txs into the mempool schema.
        let txs = gen_transfers(10);
        let alone_txs_1 = &txs[0..2];
        let batch_1 = &txs[2..4];
        let batch_2 = &txs[4..6];
        let alone_txs_2 = &txs[6..8];
        let batch_3 = &txs[8..10];

        let elements_count = alone_txs_1.len() + alone_txs_2.len() + 3; // Amount of alone txs + amount of batches.

        for tx in alone_txs_1 {
            MempoolSchema(&conn)
                .insert_tx(tx)
                .expect("Can't insert txs");
        }

        MempoolSchema(&conn)
            .insert_batch(batch_1)
            .expect("Can't insert txs");

        MempoolSchema(&conn)
            .insert_batch(batch_2)
            .expect("Can't insert txs");

        for tx in alone_txs_2 {
            MempoolSchema(&conn)
                .insert_tx(tx)
                .expect("Can't insert txs");
        }

        MempoolSchema(&conn)
            .insert_batch(batch_3)
            .expect("Can't insert txs");

        // Load the txs and check that they match the expected list.
        let txs_from_db = MempoolSchema(&conn).load_txs().expect("Can't load txs");
        assert_eq!(txs_from_db.len(), elements_count);

        assert!(matches!(txs_from_db[0], TxVariant::Tx(_)));
        assert!(matches!(txs_from_db[1], TxVariant::Tx(_)));
        assert!(matches!(txs_from_db[2], TxVariant::Batch(_)));
        assert!(matches!(txs_from_db[3], TxVariant::Batch(_)));
        assert!(matches!(txs_from_db[4], TxVariant::Tx(_)));
        assert!(matches!(txs_from_db[5], TxVariant::Tx(_)));
        assert!(matches!(txs_from_db[6], TxVariant::Batch(_)));

        Ok(())
    });
}

/// Checks that removed txs won't appear on the next load.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn remove_txs() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Point at which txs will be split into removed / retained.
        const SPLIT_TXS_AT: usize = 2;

        // Insert several txs into the mempool schema.
        let txs = franklin_txs();
        for tx in &txs {
            MempoolSchema(&conn)
                .insert_tx(tx)
                .expect("Can't insert txs");
        }

        // Remove several txs from the schema.
        let hashes_to_remove: Vec<_> = txs[SPLIT_TXS_AT..]
            .iter()
            .map(|tx| tx.hash().as_ref().to_vec())
            .collect();
        let retained_hashes: Vec<_> = txs[..SPLIT_TXS_AT].iter().map(|tx| tx.hash()).collect();
        for hash in hashes_to_remove {
            MempoolSchema(&conn)
                .remove_tx(&hash)
                .expect("Can't remove txs");
        }

        // Load the txs and check that they match the expected list.
        let txs_from_db = MempoolSchema(&conn).load_txs().expect("Can't load txs");
        assert_eq!(txs_from_db.len(), retained_hashes.len());

        for (expected_hash, tx_from_db) in retained_hashes.iter().zip(txs_from_db) {
            assert_eq!(*expected_hash, unwrap_tx(tx_from_db).hash());
        }

        Ok(())
    });
}

/// Checks that already committed txs are removed by `collect_garbage` method.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn collect_garbage() {
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        // Insert several txs into the mempool schema.
        let txs = franklin_txs();
        for tx in &txs {
            MempoolSchema(&conn)
                .insert_tx(tx)
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
        };
        OperationsSchema(&conn).store_executed_operation(executed_tx)?;

        // Collect the garbage. Execution transaction (very first one from the list)
        // should be removed from the schema.
        MempoolSchema(&conn)
            .collect_garbage()
            .expect("Can't collect the garbage");
        let retained_hashes: Vec<_> = txs[1..].iter().map(|tx| tx.hash()).collect();

        // Load the txs and check that they match the expected list.
        let txs_from_db = MempoolSchema(&conn).load_txs().expect("Can't load txs");
        assert_eq!(txs_from_db.len(), retained_hashes.len());

        for (expected_hash, tx_from_db) in retained_hashes.iter().zip(txs_from_db) {
            assert_eq!(*expected_hash, unwrap_tx(tx_from_db).hash());
        }

        Ok(())
    });
}
