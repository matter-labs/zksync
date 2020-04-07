//! Observer mode continuously checks the database and keeps updated state of the accounts in memory.
//! The state is then fed to other actors when server transitions to the leader mode.

use std::time::Duration;
use std::thread;
use std::sync::mpsc;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::node::{BlockNumber, FranklinOp, AccountTree, Account};
use circuit::witness::deposit::apply_deposit_tx;
use circuit::witness::transfer::apply_transfer_tx;
use circuit::witness::transfer_to_new::apply_transfer_to_new_tx;
use circuit::witness::withdraw::apply_withdraw_tx;
use circuit::witness::close_account::apply_close_account_tx;
use circuit::witness::full_exit::apply_full_exit_tx;
use circuit::witness::change_pubkey_offchain::apply_change_pubkey_offchain_tx;

pub fn run(conn_pool: storage::ConnectionPool, interval: Duration, stop: mpsc::Receiver<()>) -> (BlockNumber, CircuitAccountTree, AccountTree) {
    let storage = conn_pool.access_storage().expect("failed to access db");
    let mut circuit_account_tree = CircuitAccountTree::new(models::params::account_tree_depth() as u32);
    let mut account_tree = AccountTree::new(models::params::account_tree_depth() as u32);
    let mut block_number = init_circuit_account_tree(&storage, &mut circuit_account_tree).expect("failed to init circuit_account_tree");
    init_account_tree(&storage, &mut account_tree, block_number).expect("failed to initialize account tree");
    loop {
        let exit = match stop.try_recv() {
            Err(mpsc::TryRecvError::Empty) => false,
            Err(e) => {
                panic!("stop channel recv error: {}", e);
            },
            Ok(_) => true
        };
        thread::sleep(interval);
        let from_block = block_number;
        block_number = update_circuit_account_tree_from_db(&storage, &mut circuit_account_tree, from_block).expect("failed state update in observer mode");
        update_account_tree_from_db(&storage, &mut account_tree, from_block, block_number).expect("failed state update in observer mode");
        if exit {
            return (block_number, circuit_account_tree, account_tree);
        }
    }
}

fn init_circuit_account_tree(storage: &storage::StorageProcessor, account_tree: &mut CircuitAccountTree) -> Result<BlockNumber, failure::Error> {
    let (block_number, accounts) = storage
        .chain()
        .state_schema()
        .load_committed_state(None)
        .map_err(|e| failure::format_err!("couldn't load commited state: {}", e))?;
    for (account_id, account) in accounts.into_iter() {
        let circuit_account = CircuitAccount::from(account);
        account_tree.insert(account_id, circuit_account);
    }
    Ok(block_number)
}

fn init_account_tree(storage: &storage::StorageProcessor, account_tree: &mut AccountTree, block_number: BlockNumber) -> Result<(), failure::Error> {
    let (_, accounts) = storage
        .chain()
        .state_schema()
        .load_committed_state(Some(block_number))
        .expect("failed to load committed state from db");
    for (id, account) in accounts {
        account_tree.insert(id, account);
    }
    Ok(())
}

fn update_circuit_account_tree_from_db(storage: &storage::StorageProcessor, account_tree: &mut CircuitAccountTree, from_block: BlockNumber) -> Result<BlockNumber, failure::Error> {
    let block_number = storage
        .chain()
        .block_schema()
        .get_last_committed_block()
        .map_err(|e| failure::format_err!("failed to get last committed block: {}", e))?;
    
    for bn in from_block..block_number {
        let ops = storage
        .chain()
        .block_schema()
        .get_block_operations(bn+1)
        .map_err(|e| failure::format_err!("failed to get block operations {}", e))?;
        for op in ops {
            match op {
                FranklinOp::Deposit(deposit) => {
                    apply_deposit_tx(account_tree, &deposit);
                }
                FranklinOp::Transfer(transfer) => {
                    apply_transfer_tx(account_tree, &transfer);
                }
                FranklinOp::TransferToNew(transfer_to_new) => {
                    apply_transfer_to_new_tx(account_tree, &transfer_to_new);
                }
                FranklinOp::Withdraw(withdraw) => {
                    apply_withdraw_tx(account_tree, &withdraw);
                }
                FranklinOp::Close(close) => {
                    apply_close_account_tx(account_tree, &close);
                }
                FranklinOp::FullExit(full_exit_op) => {
                    let success = full_exit_op.withdraw_amount.is_some();
                    apply_full_exit_tx(account_tree, &full_exit_op, success);
                }
                FranklinOp::ChangePubKeyOffchain(change_pkhash_op) => {
                    apply_change_pubkey_offchain_tx(
                        account_tree,
                        &change_pkhash_op,
                    );
                }
                FranklinOp::Noop(_) => {}
            }
        }
    }

    Ok(block_number)
}

fn update_account_tree_from_db(storage: &storage::StorageProcessor, account_tree: &mut AccountTree, from_block: BlockNumber, to_block: BlockNumber) -> Result<(), failure::Error> {
    let state_diff = storage
        .chain()
        .state_schema()
        .load_state_diff(from_block, Some(to_block))
        .map_err(|e| failure::format_err!("failed to load committed state: {}", e))?;

    if let Some((_, updates)) = state_diff {
        for (id, update) in updates.into_iter() {
            let updated_account = Account::apply_update(account_tree.remove(id), update);
            if let Some(account) = updated_account {
                account_tree.insert(id, account);
            }
        }
    }
    Ok(())
}
