//! Observer mode continuously checks the database and keeps updated state of the accounts in memory.
//! The state is then fed to other actors when server transitions to the leader mode.

use crate::state_keeper::PlasmaStateInitParams;
use circuit::witness::{
    ChangePubkeyOffChainWitness, CloseAccountWitness, DepositWitness, FullExitWitness,
    TransferToNewWitness, TransferWitness, WithdrawWitness, Witness,
};
use log::info;
use models::node::{BlockNumber, FranklinOp};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use zksync_crypto::circuit::account::CircuitAccount;
use zksync_crypto::circuit::CircuitAccountTree;

/// The state being observed during observer mode. Meant to be used later to initialize server actors.
pub struct ObservedState {
    /// Used to initialize `PlasmaStateKeeper`
    pub state_keeper_init: PlasmaStateInitParams,
    /// Used to initialize pool of prover_server.
    pub circuit_acc_tree: CircuitAccountTree,
    /// Block number corresponding to the state in `circuit_acc_tree`.
    pub circuit_tree_block: BlockNumber,

    pub connection_pool: storage::ConnectionPool,
}

impl ObservedState {
    fn new(connection_pool: storage::ConnectionPool) -> Self {
        Self {
            state_keeper_init: PlasmaStateInitParams::new(),
            circuit_acc_tree: CircuitAccountTree::new(zksync_crypto::params::account_tree_depth()),
            circuit_tree_block: 0,
            connection_pool,
        }
    }

    /// Init state by pulling verified and committed state from db.
    async fn init(&mut self) -> Result<(), failure::Error> {
        self.init_circuit_tree().await?;
        info!("updated circuit tree to block: {}", self.circuit_tree_block);
        let mut storage = self.connection_pool.access_storage().await?;
        self.state_keeper_init = PlasmaStateInitParams::restore_from_db(&mut storage).await?;
        info!(
            "updated state keeper init params to block: {}",
            self.state_keeper_init.last_block_number
        );
        Ok(())
    }

    async fn init_circuit_tree(&mut self) -> Result<(), failure::Error> {
        let mut storage = self.connection_pool.access_storage().await?;

        let (block_number, accounts) =
            storage
                .chain()
                .state_schema()
                .load_verified_state()
                .await
                .map_err(|e| failure::format_err!("couldn't load committed state: {}", e))?;
        for (account_id, account) in accounts.into_iter() {
            let circuit_account = CircuitAccount::from(account.clone());
            self.circuit_acc_tree.insert(account_id, circuit_account);
        }
        self.circuit_tree_block = block_number;
        Ok(())
    }

    /// Pulls new changes from db and update.
    async fn update(&mut self) -> Result<(), failure::Error> {
        let old = self.circuit_tree_block;
        self.update_circuit_account_tree().await?;
        if old != self.circuit_tree_block {
            info!("updated circuit tree to block: {}", self.circuit_tree_block);
        }
        let old = self.state_keeper_init.last_block_number;

        let mut storage = self.connection_pool.access_storage().await?;
        self.state_keeper_init.load_state_diff(&mut storage).await?;
        if old != self.state_keeper_init.last_block_number {
            info!(
                "updated state keeper init params to block: {}",
                self.state_keeper_init.last_block_number
            );
        }
        Ok(())
    }

    async fn update_circuit_account_tree(&mut self) -> Result<(), failure::Error> {
        let block_number = {
            let mut storage = self.connection_pool.access_storage().await?;
            storage
                .chain()
                .block_schema()
                .get_last_verified_block()
                .await
                .map_err(|e| failure::format_err!("failed to get last committed block: {}", e))?
        };

        for bn in self.circuit_tree_block..block_number {
            let ops = {
                let mut storage = self.connection_pool.access_storage().await?;
                storage
                    .chain()
                    .block_schema()
                    .get_block_operations(bn + 1)
                    .await
                    .map_err(|e| failure::format_err!("failed to get block operations {}", e))?
            };
            self.apply(ops);
        }
        self.circuit_tree_block = block_number;
        Ok(())
    }

    fn apply(&mut self, ops: Vec<FranklinOp>) {
        for op in ops {
            match op {
                FranklinOp::Deposit(deposit) => {
                    DepositWitness::apply_tx(&mut self.circuit_acc_tree, &deposit);
                }
                FranklinOp::Transfer(transfer) => {
                    TransferWitness::apply_tx(&mut self.circuit_acc_tree, &transfer);
                }
                FranklinOp::TransferToNew(transfer_to_new) => {
                    TransferToNewWitness::apply_tx(&mut self.circuit_acc_tree, &transfer_to_new);
                }
                FranklinOp::Withdraw(withdraw) => {
                    WithdrawWitness::apply_tx(&mut self.circuit_acc_tree, &withdraw);
                }
                FranklinOp::Close(close) => {
                    CloseAccountWitness::apply_tx(&mut self.circuit_acc_tree, &close);
                }
                FranklinOp::FullExit(full_exit_op) => {
                    let success = full_exit_op.withdraw_amount.is_some();
                    FullExitWitness::apply_tx(
                        &mut self.circuit_acc_tree,
                        &(*full_exit_op, success),
                    );
                }
                FranklinOp::ChangePubKeyOffchain(change_pkhash_op) => {
                    ChangePubkeyOffChainWitness::apply_tx(
                        &mut self.circuit_acc_tree,
                        &change_pkhash_op,
                    );
                }
                FranklinOp::Noop(_) => {}
            }
        }
    }
}

/// Accumulate state from db continuously and return that state on stop signal.
///
/// # Panics
/// Panics on failed connection to db.
pub async fn run(
    conn_pool: storage::ConnectionPool,
    interval: Duration,
    stop: mpsc::Receiver<()>,
) -> ObservedState {
    info!("starting observer mode");
    let mut observed_state = ObservedState::new(conn_pool);
    observed_state
        .init()
        .await
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
            .await
            .expect("failed to update observed state");
        if exit {
            break;
        }
    }
    observed_state
}
