use plasma::state::{OpSuccess, PlasmaState};

use crate::franklin_ops::FranklinOpsBlock;
use crate::helpers::{DataRestoreError, DATA_RESTORE_CONFIG};

use models::node::account::{Account, AccountAddress};
use models::node::operations::FranklinOp;
use models::node::priority_ops::FranklinPriorityOp;
use models::node::tx::FranklinTx;
use models::node::{AccountId, AccountMap, AccountUpdates, Fr};

/// Franklin Accounts states with data restore configuration
pub struct FranklinAccountsState {
    /// Accounts stored in a spase Merkle tree and current block number
    pub state: PlasmaState,
    pub fee_account_address: AccountAddress,
}

impl FranklinAccountsState {
    pub fn new() -> Self {
        Self {
            state: PlasmaState::empty(),
            fee_account_address: DATA_RESTORE_CONFIG.fee_account_address.clone(),
        }
    }

    fn new_test() -> Self {
        Self {
            state: PlasmaState::empty(),
            fee_account_address: AccountAddress {
                data: [08, 09, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 34, 25, 26, 27]
            },
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
        let operations = block.ops.clone();

        let mut accounts_updated = Vec::new();
        let mut fees = Vec::new();

        for operation in operations {
            match operation {
                FranklinOp::Deposit(_op) => {
                    let OpSuccess {
                        fee,
                        mut updates,
                        executed_op: _,
                    } = self
                        .state
                        .execute_priority_op(FranklinPriorityOp::Deposit(_op.priority_op));
                    if let Some(fee) = fee {
                        fees.push(fee);
                    }
                    accounts_updated.append(&mut updates);
                }
                FranklinOp::TransferToNew(mut _op) => {
                    let from = self
                        .state
                        .get_account(_op.from)
                        .ok_or(DataRestoreError::WrongData("Nonexistent account".to_string()))?;
                    _op.tx.from = from.address;
                    // _op.tx.nonce = from.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op: _,
                    }) = self.state.execute_tx(FranklinTx::Transfer(_op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                }
                FranklinOp::Withdraw(mut _op) => {
                    // Withdraw op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(_op.account_id)
                        .ok_or(DataRestoreError::WrongData("Nonexistent account".to_string()))?;
                    _op.tx.account = account.address;
                    // _op.tx.nonce = account.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op: _,
                    }) = self.state.execute_tx(FranklinTx::Withdraw(_op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                }
                FranklinOp::Close(mut _op) => {
                    // Close op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(_op.account_id)
                        .ok_or(DataRestoreError::WrongData("Nonexistent account".to_string()))?;
                    _op.tx.account = account.address;
                    // _op.tx.nonce = account.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op: _,
                    }) = self.state.execute_tx(FranklinTx::Close(_op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                }
                FranklinOp::Transfer(mut _op) => {
                    let from = self
                        .state
                        .get_account(_op.from)
                        .ok_or(DataRestoreError::WrongData("Nonexistent account".to_string()))?;
                    let to = self
                        .state
                        .get_account(_op.to)
                        .ok_or(DataRestoreError::WrongData("Nonexistent account".to_string()))?;
                    _op.tx.from = from.address;
                    _op.tx.to = to.address;
                    // _op.tx.nonce = from.nonce + 1;
                    if let Ok(OpSuccess {
                        fee,
                        mut updates,
                        executed_op: _,
                    }) = self.state.execute_tx(FranklinTx::Transfer(_op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                }
                FranklinOp::FullExit(_op) => {
                    println!("fe {:?}", &_op);
                    let OpSuccess {
                        fee,
                        mut updates,
                        executed_op: _,
                    } = self
                        .state
                        .execute_priority_op(FranklinPriorityOp::FullExit(_op.priority_op));
                    if let Some(fee) = fee {
                        fees.push(fee);
                    }
                    accounts_updated.append(&mut updates);
                }
                _ => {}
            }
        }

        // let (_, fee_updates) = self.state.collect_fee(&fees, &self.fee_account_address);
        // accounts_updated.extend(fee_updates.into_iter());

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

#[cfg(test)]
mod test {
    use models::node::account::{Account, AccountAddress};
    use crate::franklin_ops::FranklinOpsBlock;
    use crate::accounts_state::FranklinAccountsState;
    #[test]
    fn test_tree_update() {
        let data1 = "0100000000000000000000000000041336c4e56f98000809101112131415161718192021222334252627000000000000";
        let decoded1 = hex::decode(data1).expect("Decoding failed");
        let ops1 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded1)
            .expect("cant get ops from data 1");
        println!("ops1 {:?} \n", &ops1);
        let block1 = FranklinOpsBlock {
            block_num: 1,
            ops: ops1,
        };

        let data2 = "030000000000000000000000000002c68af0bb14000000005711e991397fca8f5651c9bb6fa06b57e4a4dcc000000000";
        let decoded2 = hex::decode(data2).expect("Decoding failed");
        let ops2 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded2)
            .expect("cant get ops from data 2");
        println!("ops2 {:?} \n", &ops2);
        let block2 = FranklinOpsBlock {
            block_num: 2,
            ops: ops2,
        };
        
        let mut tree = FranklinAccountsState::new_test();
        let updates1 = tree.update_accounts_states_from_ops_block(&block1).expect("Cant update state from block 1");
        println!("updates1 {:?} \n", updates1);
        println!("root hash 1 {:?} \n", tree.root_hash());
        println!("accounts 1 {:?} \n", tree.get_accounts());
        let updates2 = tree.update_accounts_states_from_ops_block(&block2).expect("Cant update state from block 2");
        println!("updates2 {:?} \n", updates2);
        println!("root hash 2 {:?} \n", tree.root_hash());
        println!("accounts 2 {:?} \n", tree.get_accounts());
    }
}