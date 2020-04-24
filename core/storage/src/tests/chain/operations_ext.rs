// Built-in imports
use std::collections::HashMap;
// External imports
use num::BigUint;
// Workspace imports
use crypto_exports::franklin_crypto::bellman::pairing::ff::Field;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::operations::{ChangePubKeyOp, FranklinOp};
use models::node::priority_ops::PriorityOp;
use models::node::{
    Address, CloseOp, Deposit, DepositOp, Fr, FullExit, FullExitOp, Token, TransferOp,
    TransferToNewOp, WithdrawOp,
};
use testkit::zksync_account::ZksyncAccount;
// Local imports
use crate::tests::db_test;
use crate::StorageProcessor;

/// Here we take the account transactions using `get_account_transactions` and
/// check `get_account_transactions_history` to match obtained results.
#[test]
#[cfg_attr(not(feature = "db_test"), ignore)]
fn get_account_transactions_history() {
    let tokens = vec![
        Token::new(0, Address::zero(), "ETH"),   // used for deposits
        Token::new(1, Address::random(), "DAI"), // used for transfers
        Token::new(2, Address::random(), "FAU"), // used for withdraws
    ];

    let from_zksync_account = ZksyncAccount::rand();
    let from_account_id = 0xbabe;
    let from_account_address = from_zksync_account.address;
    let from_account_address_string = format!("{:?}", &from_account_address);

    let to_zksync_account = ZksyncAccount::rand();
    let to_account_id = 0xdcba;
    let to_account_address = to_zksync_account.address;
    let to_account_address_string = format!("{:?}", &to_account_address);

    let amount = BigUint::from(1u32);

    let executed_deposit_op = {
        let deposit_op = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op: Deposit {
                from: from_account_address,
                token: tokens[0].id,
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
                eth_hash: b"1234567890".to_vec(),
            },
            op: deposit_op,
            block_index: 31,
        };

        ExecutedOperations::PriorityOp(Box::new(executed_op))
    };

    let executed_full_exit_op = {
        let full_exit_op = FranklinOp::FullExit(Box::new(FullExitOp {
            priority_op: FullExit {
                account_id: from_account_id,
                eth_address: from_account_address,
                token: tokens[2].id,
            },
            withdraw_amount: Some(amount.clone()),
        }));

        let executed_op = ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: 0,
                data: full_exit_op.try_get_priority_op().unwrap(),
                deadline_block: 0,
                eth_hash: b"1234567890".to_vec(),
            },
            op: full_exit_op,
            block_index: 31,
        };

        ExecutedOperations::PriorityOp(Box::new(executed_op))
    };

    let executed_transfer_to_new_op = {
        let transfer_to_new_op = FranklinOp::TransferToNew(Box::new(TransferToNewOp {
            tx: from_zksync_account
                .sign_transfer(
                    tokens[1].id,
                    &tokens[1].symbol,
                    amount.clone(),
                    BigUint::from(0u32),
                    &to_account_address,
                    None,
                    true,
                )
                .0,
            from: from_account_id,
            to: to_account_id,
        }));

        let executed_transfer_to_new_op = ExecutedTx {
            tx: transfer_to_new_op.try_get_tx().unwrap(),
            success: true,
            op: Some(transfer_to_new_op),
            fail_reason: None,
            block_index: None,
        };

        ExecutedOperations::Tx(Box::new(executed_transfer_to_new_op))
    };

    let executed_transfer_op = {
        let transfer_op = FranklinOp::Transfer(Box::new(TransferOp {
            tx: from_zksync_account
                .sign_transfer(
                    tokens[1].id,
                    &tokens[1].symbol,
                    amount.clone(),
                    BigUint::from(0u32),
                    &to_account_address,
                    None,
                    true,
                )
                .0,
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
            tx: from_zksync_account
                .sign_withdraw(
                    tokens[2].id,
                    &tokens[2].symbol,
                    amount.clone(),
                    BigUint::from(0u32),
                    &to_account_address,
                    None,
                    true,
                )
                .0,
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

    let executed_close_op = {
        let close_op = FranklinOp::Close(Box::new(CloseOp {
            tx: from_zksync_account.sign_close(None, false),
            account_id: from_account_id,
        }));

        let executed_close_op = ExecutedTx {
            tx: close_op.try_get_tx().unwrap(),
            success: true,
            op: Some(close_op),
            fail_reason: None,
            block_index: None,
        };

        ExecutedOperations::Tx(Box::new(executed_close_op))
    };

    let executed_change_pubkey_op = {
        let change_pubkey_op = FranklinOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
            tx: from_zksync_account.create_change_pubkey_tx(None, false, false),
            account_id: from_account_id,
        }));

        let executed_change_pubkey_op = ExecutedTx {
            tx: change_pubkey_op.try_get_tx().unwrap(),
            success: true,
            op: Some(change_pubkey_op),
            fail_reason: None,
            block_index: None,
        };

        ExecutedOperations::Tx(Box::new(executed_change_pubkey_op))
    };

    let block = Block {
        block_number: 1,
        new_root_hash: Fr::zero(),
        fee_account: 0,
        block_transactions: vec![
            executed_deposit_op,
            executed_full_exit_op,
            executed_transfer_to_new_op,
            executed_transfer_op,
            executed_withdraw_op,
            executed_close_op,
            executed_change_pubkey_op,
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
                Some(tokens[0].symbol.clone()),
                Some(amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Transfer",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(tokens[1].symbol.clone()),
                Some(amount.to_string()),
            ),
        );
        expected_behavior.insert(
            "Withdraw",
            (
                Some(from_account_address_string.as_str()),
                Some(to_account_address_string.as_str()),
                Some(tokens[2].symbol.clone()),
                Some(amount.to_string()),
            ),
        );
        expected_behavior
    };

    // execute_operation
    let conn = StorageProcessor::establish_connection().unwrap();
    db_test(conn.conn(), || {
        for token in &tokens {
            conn.tokens_schema().store_token(token.clone())?;
        }

        conn.chain().block_schema().save_block_transactions(block)?;

        let from_history = conn
            .chain()
            .operations_ext_schema()
            .get_account_transactions_history(&from_account_address, 0, 10)?;

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
            .get_account_transactions_history(&to_account_address, 0, 10)?;

        assert_eq!(from_history.len(), 7);
        assert_eq!(to_history.len(), 4);

        Ok(())
    });
}
