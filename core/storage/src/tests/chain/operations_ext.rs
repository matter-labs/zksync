// Built-in imports
use std::collections::HashMap;
// External imports
use bigdecimal::BigDecimal;
// Workspace imports
use crypto_exports::franklin_crypto::bellman::pairing::ff::Field;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::operations::FranklinOp;
use models::node::priority_ops::PriorityOp;
use models::node::{Deposit, DepositOp, Fr, TransferOp, WithdrawOp};
use testkit::zksync_account::ZksyncAccount;
// Local imports
use crate::tests::db_test;
use crate::StorageProcessor;

/// Here we take the account transactions using `get_account_transactions` and
/// check `get_account_transactions_history` to match obtained results.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn get_account_transactions_history() {
    let from_zksync_account = ZksyncAccount::rand();
    let from_account_id = 0xbabe;
    let from_account_address = from_zksync_account.address;
    let from_account_address_string = format!("{:?}", &from_account_address);

    let to_zksync_account = ZksyncAccount::rand();
    let to_account_id = 0xdcba;
    let to_account_address = to_zksync_account.address;
    let to_account_address_string = format!("{:?}", &to_account_address);

    let token = 0;
    let amount = BigDecimal::from(1);

    let executed_deposit_op = {
        let deposit_op = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op: Deposit {
                from: from_account_address,
                token,
                amount: amount.clone(),
                to: to_account_address,
            },
            account_id: from_account_id,
        }));

        let executed_op = ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: 0,
                data: deposit_op.try_get_priority_op().unwrap(),
                deadline_block: 0,
                eth_fee: 0.into(),
                eth_hash: b"1234567890".to_vec(),
            },
            op: deposit_op,
            block_index: 31,
        };

        ExecutedOperations::PriorityOp(Box::new(executed_op))
    };

    let executed_transfer_op = {
        let transfer_op = FranklinOp::Transfer(Box::new(TransferOp {
            tx: from_zksync_account.sign_transfer(
                token,
                amount.clone(),
                BigDecimal::from(0),
                &to_account_address,
                None,
                true,
            ),
            from: from_account_id,
            to: to_account_id,
        }));

        let executed_transfer_op = ExecutedTx {
            tx: transfer_op.try_get_tx().unwrap(),
            success: true,
            op: Some(transfer_op),
            fail_reason: None,
            block_index: None,
        };

        ExecutedOperations::Tx(Box::new(executed_transfer_op))
    };

    let executed_withdraw_op = {
        let withdraw_op = FranklinOp::Withdraw(Box::new(WithdrawOp {
            tx: from_zksync_account.sign_withdraw(
                token,
                amount.clone(),
                BigDecimal::from(0),
                &from_account_address,
                None,
                true,
            ),
            account_id: from_account_id,
        }));

        let executed_withdraw_op = ExecutedTx {
            tx: withdraw_op.try_get_tx().unwrap(),
            success: true,
            op: Some(withdraw_op),
            fail_reason: None,
            block_index: None,
        };

        ExecutedOperations::Tx(Box::new(executed_withdraw_op))
    };

    let block = Block {
        block_number: 1,
        new_root_hash: Fr::zero(),
        fee_account: 0,
        block_transactions: vec![
            executed_deposit_op,
            executed_transfer_op,
            executed_withdraw_op,
        ],
        processed_priority_ops: (0, 0), // Not important
    };

    let expected_behavior = {
        let mut expected_behavior = HashMap::new();
        expected_behavior.insert(
            "Deposit",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(&token),
                Some(amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Transfer",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(&token),
                Some(amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Withdraw",
            (
                Some(from_account_address_string.as_str()),
                Some(from_account_address_string.as_str()),
                Some(&token),
                Some(amount.to_string()),
            ),
        );
        expected_behavior
    };

    // execute_operation
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        conn.chain().block_schema().save_block_transactions(block)?;

        let from_history = conn
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history(&from_account_address, 0, 10)?;

        for tx in &from_history {
            let tx_type: &str = tx.tx["type"].as_str().expect("no tx_type");
            let (from, to, token, amount) = expected_behavior
                .get(tx_type)
                .expect("no expected behavior");

            let tx_info = match tx_type {
                "Deposit" => tx.tx["priority_op"].clone(),
                _ => tx.tx.clone(),
            };
            let tx_from_addr = tx_info["from"].as_str();
            let tx_to_addr = tx_info["to"].as_str();
            let tx_token = tx_info["token"].as_u64().map(|x| x as u16);
            let tx_amount = tx_info["amount"].as_str().map(String::from);

            assert!(tx.hash.is_some());

            assert_eq!(tx_from_addr, *from);
            assert_eq!(tx_to_addr, *to);
            assert_eq!(tx_token, token.cloned());
            assert_eq!(tx_amount, *amount);
        }

        let to_history = conn
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history(&to_account_address, 0, 10)?;

        assert_eq!(from_history.len(), 3);
        assert_eq!(to_history.len(), 2);

        Ok(())
    });
}
