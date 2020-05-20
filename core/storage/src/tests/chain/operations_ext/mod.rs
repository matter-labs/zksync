// Built-in imports
use std::collections::HashMap;
// External imports
// Workspace imports
// Local imports
use self::setup::TransactionsHistoryTestSetup;
use crate::tests::db_test;
use crate::StorageProcessor;

mod setup;

/// Here we take the account transactions using `get_account_transactions` and
/// check `get_account_transactions_history` to match obtained results.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn get_account_transactions_history() {
    let mut setup = TransactionsHistoryTestSetup::new();
    setup.add_block(1);

    let from_account_address_string = format!("{:?}", setup.from_zksync_account.address);
    let to_account_address_string = format!("{:?}", setup.to_zksync_account.address);

    let expected_behavior = {
        let mut expected_behavior = HashMap::new();
        expected_behavior.insert(
            "Deposit",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(setup.tokens[0].symbol.clone()),
                Some(setup.amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Transfer",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(setup.tokens[1].symbol.clone()),
                Some(setup.amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Withdraw",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(setup.tokens[2].symbol.clone()),
                Some(setup.amount.to_string()),
            ),
        );
        expected_behavior
    };

    // execute_operation
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        for token in &setup.tokens {
            conn.tokens_schema().store_token(token.clone())?;
        }

        for block in setup.blocks {
            conn.chain().block_schema().save_block_transactions(block)?;
        }

        let from_history = conn
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history(&setup.from_zksync_account.address, 0, 10)?;

        for tx in &from_history {
            let tx_type: &str = tx.tx["type"].as_str().expect("no tx_type");

            assert!(tx.hash.is_some());

            if let Some((from, to, token, amount)) = expected_behavior.get(tx_type) {
                let tx_info = match tx_type {
                    "Deposit" | "FullExit" => tx.tx["priority_op"].clone(),
                    _ => tx.tx.clone(),
                };
                let tx_from_addr = tx_info["from"].as_str();
                let tx_to_addr = tx_info["to"].as_str();
                let tx_token = tx_info["token"].as_str().map(String::from);
                let tx_amount = tx_info["amount"].as_str().map(String::from);

                assert_eq!(tx_from_addr, *from);
                assert_eq!(tx_to_addr, *to);
                assert_eq!(tx_token, *token);
                assert_eq!(tx_amount, *amount);
            }
        }

        let to_history = conn
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history(&setup.to_zksync_account.address, 0, 10)?;

        assert_eq!(from_history.len(), 7);
        assert_eq!(to_history.len(), 4);

        Ok(())
    });
}
