use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::convert::TryInto;

use ff::{Field, PrimeField, PrimeFieldRepr};
use plasma::state::{OpSuccess, PlasmaState};

use crate::franklin_ops::FranklinOpsBlock;
use crate::helpers::{
    DATA_RESTORE_CONFIG,
    DataRestoreError,
    get_topic_keccak_hash
};

use models::node::operations::{
    TX_TYPE_BYTES_LEGTH, DepositOp, FranklinOp, FullExitOp, TransferOp, TransferToNewOp, WithdrawOp,
};
use models::node::priority_ops::{Deposit, FranklinPriorityOp, FullExit};
use models::node::tx::{Close, FranklinTx, Transfer, Withdraw};
use models::node::{AccountMap, Fr, AccountId, AccountUpdates};
use models::node::account::{Account, AccountAddress, AccountUpdate};

/// Franklin Accounts states with data restore configuration
pub struct FranklinAccountsState {
    /// Accounts stored in a spase Merkle tree and current block number
    pub state: PlasmaState,
    pub fee_account_address: AccountAddress
}

impl FranklinAccountsState {
    pub fn new() -> Self {
        Self {
            state: PlasmaState::empty(),
            fee_account_address: DATA_RESTORE_CONFIG.fee_account_address.clone(),
        }
    }

    /// Creates empty Franklin Accounts states
    pub fn load(accounts: AccountMap, current_block: u32) -> Self {
        Self {
            state: PlasmaState::new(accounts, current_block + 1),
            fee_account_address: DATA_RESTORE_CONFIG.fee_account_address.clone(),
        }
    }

    /// Updates Franklin Accounts states from Franklin op
    ///
    /// # Arguments
    ///
    /// * `op` - Franklin operation
    /// 
    pub fn update_accounts_states_from_ops_block(
        &mut self,
        block: &FranklinOpsBlock,
    ) -> Result<AccountUpdates, DataRestoreError> {
        let mut operations = block.ops.clone();

        let mut accounts_updated = Vec::new();
        let mut fees = Vec::new();

        for operation in operations {
            match operation {
                FranklinOp::Deposit(_op) => {
                    let OpSuccess {
                        fee,
                        mut updates,
                        executed_op,
                    } = self.state.execute_priority_op(
                        FranklinPriorityOp::Deposit(_op.priority_op)
                    );
                    if let Some(fee) = fee {
                        fees.push(fee);
                    }
                    accounts_updated.append(&mut updates);
                },
                FranklinOp::TransferToNew(mut _op) => {
                    let from = self.state.get_account(_op.from)
                        .ok_or(DataRestoreError::NonexistentAccount)?;
                    _op.tx.from = from.address;
                    _op.tx.nonce = from.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op,
                    }) = self.state.execute_tx(
                        FranklinTx::Transfer(_op.tx)
                    ) {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                },
                FranklinOp::Withdraw(mut _op) => {
                    // Withdraw op comes with empty Account Address and Nonce fields
                    let account = self.state.get_account(_op.account_id)
                        .ok_or(DataRestoreError::NonexistentAccount)?;
                    _op.tx.account = account.address;
                    _op.tx.nonce = account.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op,
                    }) = self.state.execute_tx(
                        FranklinTx::Withdraw(_op.tx)
                    ) {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                },
                FranklinOp::Close(mut _op) => {
                    // Close op comes with empty Account Address and Nonce fields
                    let account = self.state.get_account(_op.account_id)
                        .ok_or(DataRestoreError::NonexistentAccount)?;
                    _op.tx.account = account.address;
                    _op.tx.nonce = account.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op,
                    }) = self.state.execute_tx(
                        FranklinTx::Close(_op.tx)
                    ) {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                },
                FranklinOp::Transfer(mut _op) => {
                    let from = self.state.get_account(_op.from)
                        .ok_or(DataRestoreError::NonexistentAccount)?;
                    let to = self.state.get_account(_op.to)
                        .ok_or(DataRestoreError::NonexistentAccount)?;
                    _op.tx.from = from.address;
                    _op.tx.to = to.address;
                    _op.tx.nonce = from.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op,
                    }) = self.state.execute_tx(
                        FranklinTx::Transfer(_op.tx)
                    ) {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                },
                FranklinOp::FullExit(_op) => {
                    let OpSuccess {
                        fee,
                        mut updates,
                        executed_op,
                    } = self.state.execute_priority_op(
                        FranklinPriorityOp::FullExit(_op.priority_op)
                    );
                    if let Some(fee) = fee {
                        fees.push(fee);
                    }
                    accounts_updated.append(&mut updates);
                },
            }
        }

        let (_, fee_updates) = self.state.collect_fee(&fees, &self.fee_account_address);
        accounts_updated.extend(fee_updates.into_iter());

        Ok(accounts_updated)
    }

    /// Returns map of Franklin accounts ids and their descriptions
    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.state.get_accounts()
    }

    /// Returns sparse Merkle tree root hash
    pub fn root_hash(&self) -> Fr {
        self.state.root_hash()
    }

    /// Returns Franklin Account description by its id
    pub fn get_account_by_address(&self, address: &AccountAddress) -> Option<(AccountId, Account)> {
        self.state.get_account_by_address(address)
    }
}
