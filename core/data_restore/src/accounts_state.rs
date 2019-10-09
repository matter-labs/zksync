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
    /// Accounts stored in a spase merkle tree
    pub state: PlasmaState,
}

impl FranklinAccountsState {
    /// Returns new FranklinAccountsState instance
    pub fn new() -> Self {
        Self {
            state: PlasmaState::empty(),
        }
    }

    /// Returns new FranklinAccountsState instance for tests
    fn new_test() -> Self {
        Self {
            state: PlasmaState::empty(),
        }
    }

    /// Returns FranklinAccountsState from accounts map and current block number
    /// 
    /// # Arguments
    ///
    /// * `current_block` - current block number
    /// * `accounts` - accounts map
    ///
    pub fn load(current_block: u32, accounts: AccountMap) -> Self {
        Self {
            state: PlasmaState::new(accounts, current_block),
        }
    }

    /// Updates Franklin Accounts states from Franklin operations block
    /// Returns updated accounts
    ///
    /// # Arguments
    ///
    /// * `block` - Franklin operations blocks
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
                    _op.tx.nonce = from.nonce;
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
                    _op.tx.nonce = account.nonce;
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
                    _op.tx.nonce = account.nonce;
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
                    _op.tx.nonce = from.nonce;
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
                FranklinOp::FullExit(mut _op) => {
                    _op.priority_op.nonce -= 1;
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
        let fee_account_address = self.get_account(block.fee_account)
            .ok_or(DataRestoreError::WrongData("Nonexistent fee account".to_string()))?
            .address;
        let (_, fee_updates) = self.state.collect_fee(&fees, &fee_account_address);
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

    /// Returns Franklin Account id and description by its address
    pub fn get_account_by_address(&self, address: &AccountAddress) -> Option<(AccountId, Account)> {
        self.state.get_account_by_address(address)
    }

    /// Returns Franklin Account description by its id
    pub fn get_account(&self, account_id: AccountId) -> Option<Account> {
        self.state.get_account(account_id)
    }
}

#[cfg(test)]
mod test {
    use models::node::Fr;
    use crate::franklin_ops::FranklinOpsBlock;
    use crate::accounts_state::FranklinAccountsState;

    #[test]
    fn test_tree_consistent_update() {
        let data1 = "0100000000000000000000000000041336c4e56f98000809101112131415161718192021222334252627000000000000";
        let decoded1 = hex::decode(data1).expect("Decoding failed");
        let ops1 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded1)
            .expect("cant get ops from data 1");
        let block1 = FranklinOpsBlock {
            block_num: 1,
            ops: ops1,
        };

        let data2 = "030000000000000000000000000002c68af0bb14000000005711e991397fca8f5651c9bb6fa06b57e4a4dcc000000000";
        let decoded2 = hex::decode(data2).expect("Decoding failed");
        let ops2 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded2)
            .expect("cant get ops from data 2");
        let block2 = FranklinOpsBlock {
            block_num: 2,
            ops: ops2,
        };

        let data3 = "02000000000000010008091011121314151617181920212223342526280000010000000000000000";
        let decoded3 = hex::decode(data3).expect("Decoding failed");
        let ops3 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded3)
            .expect("cant get ops from data 3");
        let block3 = FranklinOpsBlock {
            block_num: 3,
            ops: ops3,
        };

        let data4 = "05000001000000000000010000000000";
        let decoded4 = hex::decode(data4).expect("Decoding failed");
        let ops4 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded4)
            .expect("cant get ops from data 4");
        let block4 = FranklinOpsBlock {
            block_num: 4,
            ops: ops4,
        };

        let data5 = "0400000100000000";
        let decoded5 = hex::decode(data5).expect("Decoding failed");
        let ops5 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded5)
            .expect("cant get ops from data 5");
        let block5 = FranklinOpsBlock {
            block_num: 5,
            ops: ops5,
        };
        
        // FULL EXIT WILL WORK WITH SIGNATURE
        // let data3 = "06000002000000000000000000000000000000000000000000000000000000000000000052312ad6f01657413b2eae9287f6b9adad93d5fe000000000002000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000014cabd42a5b98000000";
        // let decoded3 = hex::decode(data3).expect("Decoding failed");
        // let ops3 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded3)
        //     .expect("cant get ops from data");
        // println!("ops3 {:?} \n", ops3);   
        // let block3 = FranklinOpsBlock {
        //     block_num: 3,
        //     ops: ops3,
        // };
        
        let mut tree = FranklinAccountsState::new_test();
        let updates1 = tree.update_accounts_states_from_ops_block(&block1).expect("Cant update state from block 1");
        println!("updates 1 {:?} \n", updates1);
        println!("root hash 1 {:?} \n", tree.root_hash());
        println!("accounts 1 {:?} \n", tree.get_accounts());
        let updates2 = tree.update_accounts_states_from_ops_block(&block2).expect("Cant update state from block 2");
        println!("updates 2 {:?} \n", updates2);
        println!("root hash 2 {:?} \n", tree.root_hash());
        println!("accounts 2 {:?} \n", tree.get_accounts());
        let updates3 = tree.update_accounts_states_from_ops_block(&block3).expect("Cant update state from block 3");
        println!("updates 3 {:?} \n", updates3);
        println!("root hash 3 {:?} \n", tree.root_hash());
        println!("accounts 3 {:?} \n", tree.get_accounts());
        let updates4 = tree.update_accounts_states_from_ops_block(&block4).expect("Cant update state from block 4");
        println!("updates 4 {:?} \n", updates4);
        println!("root hash 4 {:?} \n", tree.root_hash());
        println!("accounts 4 {:?} \n", tree.get_accounts());
        let updates5 = tree.update_accounts_states_from_ops_block(&block5).expect("Cant update state from block 4");
        println!("updates 5 {:?} \n", updates5);
        println!("root hash 5 {:?} \n", tree.root_hash());
        println!("accounts 5 {:?} \n", tree.get_accounts());

        assert_eq!(tree.root_hash().to_string(), "Fr(0x0f220069602ed8f8c4557fdde71baf5220bbf237790adf67f49280b84588acf2)".to_string());
    }

    #[test]
    fn test_tree_inconsistent_update() {
        let data1 = "0100000000000000000000000000041336c4e56f98000809101112131415161718192021222334252627000000000000030000000000000000000000000002c68af0bb14000000005711e991397fca8f5651c9bb6fa06b57e4a4dcc00000000002000000000000010008091011121314151617181920212223342526280000010000000000000000050000010000000000000100000000000400000100000000";
        let decoded1 = hex::decode(data1).expect("Decoding failed");
        let ops1 = FranklinOpsBlock::get_franklin_ops_from_data(&decoded1)
            .expect("cant get ops from data 1");
        let block1 = FranklinOpsBlock {
            block_num: 1,
            ops: ops1,
        };

        let mut tree = FranklinAccountsState::new_test();
        let updates1 = tree.update_accounts_states_from_ops_block(&block1).expect("Cant update state from block 1");
        println!("updates 1 {:?} \n", updates1);
        println!("root hash 1 {:?} \n", tree.root_hash());
        println!("accounts 1 {:?} \n", tree.get_accounts());

        assert_eq!(tree.root_hash().to_string(), "Fr(0x0f220069602ed8f8c4557fdde71baf5220bbf237790adf67f49280b84588acf2)".to_string());
    }
}