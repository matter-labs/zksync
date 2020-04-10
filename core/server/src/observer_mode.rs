//! Observer mode continuously checks the database and keeps updated state of the accounts in memory.
//! The state is then fed to other actors when server transitions to the leader mode.

use crate::state_keeper::PlasmaStateInitParams;
use circuit::witness::change_pubkey_offchain::apply_change_pubkey_offchain_tx;
use circuit::witness::close_account::apply_close_account_tx;
use circuit::witness::deposit::apply_deposit_tx;
use circuit::witness::full_exit::apply_full_exit_tx;
use circuit::witness::transfer::apply_transfer_tx;
use circuit::witness::transfer_to_new::apply_transfer_to_new_tx;
use circuit::witness::withdraw::apply_withdraw_tx;
use log::info;
use models::circuit::account::CircuitAccount;
use models::circuit::CircuitAccountTree;
use models::node::{BlockNumber, FranklinOp};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// The state being observed during observer mode. Meant to be used later to initialize server actors.
pub struct ObservedState {
    /// Used to initialize `PlasmaStateKeeper`
    pub state_keeper_init: PlasmaStateInitParams,
    /// Used to initialize pool of prover_server.
    pub circuit_acc_tree: CircuitAccountTree,
    /// Block number corresponding to the circuit_acc_tree.
    pub last_seen_block: BlockNumber,

    storage: storage::StorageProcessor,
}

impl ObservedState {
    fn new(storage: storage::StorageProcessor) -> Self {
        Self {
            state_keeper_init: PlasmaStateInitParams::new(),
            circuit_acc_tree: CircuitAccountTree::new(models::params::account_tree_depth()),
            last_seen_block: 0,
            storage,
        }
    }

    /// Init state by pulling verified and committed state from db.
    fn init(&mut self) -> Result<(), failure::Error> {
        self.init_circuit_tree()?;
        self.state_keeper_init.load_from_db(&self.storage)?;
        info!("updated circuit tree to block: {}", self.last_seen_block);
        info!(
            "updated state keeper init params to block: {}",
            self.state_keeper_init.last_block_number
        );
        Ok(())
    }

    fn init_circuit_tree(&mut self) -> Result<(), failure::Error> {
        let (block_number, accounts) = self
            .storage
            .chain()
            .state_schema()
            .load_verified_state()
            .map_err(|e| failure::format_err!("couldn't load committed state: {}", e))?;
        for (account_id, account) in accounts.into_iter() {
            let circuit_account = CircuitAccount::from(account.clone());
            self.circuit_acc_tree.insert(account_id, circuit_account);
        }
        self.last_seen_block = block_number;
        Ok(())
    }

    /// Pulls new changes from db and update.
    fn update(&mut self) -> Result<(), failure::Error> {
        self.update_circuit_account_tree()?;
        self.state_keeper_init.load_state_diff(&self.storage)?;
        info!("updated circuit tree to block: {}", self.last_seen_block);
        info!(
            "updated state keeper init params to block: {}",
            self.state_keeper_init.last_block_number
        );
        Ok(())
    }

    fn update_circuit_account_tree(&mut self) -> Result<(), failure::Error> {
        let block_number = self
            .storage
            .chain()
            .block_schema()
            .get_last_verified_block()
            .map_err(|e| failure::format_err!("failed to get last committed block: {}", e))?;

        for bn in self.last_seen_block..block_number {
            let ops = self
                .storage
                .chain()
                .block_schema()
                .get_block_operations(bn + 1)
                .map_err(|e| failure::format_err!("failed to get block operations {}", e))?;
            for op in ops {
                match op {
                    FranklinOp::Deposit(deposit) => {
                        apply_deposit_tx(&mut self.circuit_acc_tree, &deposit);
                    }
                    FranklinOp::Transfer(transfer) => {
                        apply_transfer_tx(&mut self.circuit_acc_tree, &transfer);
                    }
                    FranklinOp::TransferToNew(transfer_to_new) => {
                        apply_transfer_to_new_tx(&mut self.circuit_acc_tree, &transfer_to_new);
                    }
                    FranklinOp::Withdraw(withdraw) => {
                        apply_withdraw_tx(&mut self.circuit_acc_tree, &withdraw);
                    }
                    FranklinOp::Close(close) => {
                        apply_close_account_tx(&mut self.circuit_acc_tree, &close);
                    }
                    FranklinOp::FullExit(full_exit_op) => {
                        let success = full_exit_op.withdraw_amount.is_some();
                        apply_full_exit_tx(&mut self.circuit_acc_tree, &full_exit_op, success);
                    }
                    FranklinOp::ChangePubKeyOffchain(change_pkhash_op) => {
                        apply_change_pubkey_offchain_tx(
                            &mut self.circuit_acc_tree,
                            &change_pkhash_op,
                        );
                    }
                    FranklinOp::Noop(_) => {}
                }
            }
        }
        self.last_seen_block = block_number;
        Ok(())
    }
}

/// Accumulate state from db continuously and return that state on stop signal.
///
/// # Panics
/// Panics on failed connection to db.
pub fn run(
    conn_pool: storage::ConnectionPool,
    interval: Duration,
    stop: mpsc::Receiver<()>,
) -> ObservedState {
    info!("starting observer mode");
    let storage = conn_pool.access_storage().expect("failed to access db");
    let mut observed_state = ObservedState::new(storage);
    observed_state
        .init()
        .expect("failed to init observed state");
    loop {
        let exit = match stop.try_recv() {
            Err(mpsc::TryRecvError::Empty) => false,
            Err(e) => {
                panic!("stop channel recv error: {}", e);
            }
            Ok(_) => true,
        };
        thread::sleep(interval);
        observed_state
            .update()
            .expect("failed to update observed state");
        if exit {
            break;
        }
    }
    observed_state
}
