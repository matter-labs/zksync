use crate::rollup_ops::RollupOpsBlock;
use bigdecimal::BigDecimal;
use failure::format_err;
use models::node::account::Account;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::operations::FranklinOp;
use models::node::priority_ops::FranklinPriorityOp;
use models::node::priority_ops::PriorityOp;
use models::node::tx::FranklinTx;
use models::node::{AccountId, AccountMap, AccountUpdates, Fr};
use plasma::state::{CollectedFee, OpSuccess, PlasmaState};
use web3::types::Address;

/// Rollup accounts states
pub struct TreeState {
    /// Accounts stored in a spase merkle tree
    pub state: PlasmaState,
    /// Current unprocessed priority op number
    pub current_unprocessed_priority_op: u64,
    /// The last fee account address
    pub last_fee_account_address: Address,
}

impl Default for TreeState {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeState {
    /// Returns empty self state
    pub fn new() -> Self {
        Self {
            state: PlasmaState::empty(),
            current_unprocessed_priority_op: 0,
            last_fee_account_address: Address::default(),
        }
    }

    /// Returns the loaded state
    ///
    /// # Arguments
    ///
    /// * `current_block` - The current block number
    /// * `accounts` - Accounts stored in a spase merkle tree
    /// * `current_unprocessed_priority_op` - The current unprocessed priority op number
    /// * `fee_account` - The last fee account address
    ///
    pub fn load(
        current_block: u32,
        accounts: AccountMap,
        current_unprocessed_priority_op: u64,
        fee_account: AccountId,
    ) -> Self {
        let state = PlasmaState::from_acc_map(accounts, current_block);
        let last_fee_account_address = state
            .get_account(fee_account)
            .expect("Cant get fee account from tree state")
            .address;
        Self {
            state,
            current_unprocessed_priority_op,
            last_fee_account_address,
        }
    }

    /// Updates Rollup accounts states from Rollup operations block
    /// Returns current rollup block and updated accounts
    ///
    /// # Arguments
    ///
    /// * `ops_block` - Rollup operations blocks
    ///
    pub fn update_tree_states_from_ops_block(
        &mut self,
        ops_block: &RollupOpsBlock,
    ) -> Result<(Block, AccountUpdates), failure::Error> {
        let operations = ops_block.ops.clone();

        let mut accounts_updated = Vec::new();
        let mut fees = Vec::new();
        let mut ops = Vec::new();
        let mut current_op_block_index = 0u32;
        let last_unprocessed_prior_op = self.current_unprocessed_priority_op;

        for operation in operations {
            match operation {
                FranklinOp::Deposit(op) => {
                    let priority_op = FranklinPriorityOp::Deposit(op.priority_op);
                    let op_result = self.state.execute_priority_op(priority_op.clone());
                    current_op_block_index = self.update_from_priority_operation(
                        priority_op,
                        op_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops,
                    );
                }
                FranklinOp::TransferToNew(mut op) => {
                    let from = self
                        .state
                        .get_account(op.from)
                        .ok_or_else(|| format_err!("TransferToNew fail: Nonexistent account"))?;
                    op.tx.from = from.address;
                    op.tx.nonce = from.nonce;
                    let tx = FranklinTx::Transfer(Box::new(op.tx.clone()));

                    let (fee, updates) = self
                        .state
                        .apply_transfer_to_new_op(&op)
                        .map_err(|e| format_err!("TransferToNew fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee: Some(fee),
                        updates,
                        executed_op: FranklinOp::TransferToNew(op),
                    };

                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops,
                    );
                }
                FranklinOp::Withdraw(mut op) => {
                    // Withdraw op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Withdraw fail: Nonexistent account"))?;
                    op.tx.from = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = FranklinTx::Withdraw(Box::new(op.tx.clone()));
                    let (fee, updates) = self
                        .state
                        .apply_withdraw_op(&op)
                        .map_err(|e| format_err!("Withdraw fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee: Some(fee),
                        updates,
                        executed_op: FranklinOp::Withdraw(op),
                    };
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops,
                    );
                }
                FranklinOp::Close(mut op) => {
                    // Close op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Close fail: Nonexistent account"))?;
                    op.tx.account = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = FranklinTx::Close(Box::new(op.tx.clone()));
                    let (fee, updates) = self
                        .state
                        .apply_close_op(&op)
                        .map_err(|e| format_err!("Close fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee: Some(fee),
                        updates,
                        executed_op: FranklinOp::Close(op),
                    };
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops,
                    );
                }
                FranklinOp::Transfer(mut op) => {
                    let from = self
                        .state
                        .get_account(op.from)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    let to = self
                        .state
                        .get_account(op.to)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    op.tx.from = from.address;
                    op.tx.to = to.address;
                    op.tx.nonce = from.nonce;

                    let tx = FranklinTx::Transfer(Box::new(op.tx.clone()));
                    let (fee, updates) = self
                        .state
                        .apply_transfer_op(&op)
                        .map_err(|e| format_err!("Withdraw fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee: Some(fee),
                        updates,
                        executed_op: FranklinOp::Transfer(op),
                    };
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops,
                    );
                }
                FranklinOp::FullExit(op) => {
                    let priority_op = FranklinPriorityOp::FullExit(op.priority_op);
                    let op_result = self.state.execute_priority_op(priority_op.clone());
                    current_op_block_index = self.update_from_priority_operation(
                        priority_op,
                        op_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops,
                    );
                }
                FranklinOp::ChangePubKeyOffchain(mut op) => {
                    let account = self.state.get_account(op.account_id).ok_or_else(|| {
                        format_err!("ChangePubKeyOffChain fail: Nonexistent account")
                    })?;
                    op.tx.account = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = FranklinTx::ChangePubKey(Box::new(op.tx.clone()));
                    let (fee, updates) = self
                        .state
                        .apply_change_pubkey_op(&op)
                        .map_err(|e| format_err!("ChangePubKeyOffChain fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee: Some(fee),
                        updates,
                        executed_op: FranklinOp::ChangePubKeyOffchain(op),
                    };
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops,
                    );
                }
                FranklinOp::Noop(_) => {}
            }
        }

        let fee_account_address = self
            .get_account(ops_block.fee_account)
            .ok_or_else(|| format_err!("Nonexistent account"))?
            .address;

        let fee_updates = self.state.collect_fee(&fees, ops_block.fee_account);
        accounts_updated.extend(fee_updates.into_iter());

        self.last_fee_account_address = fee_account_address;

        let block = Block {
            block_number: ops_block.block_num,
            new_root_hash: self.state.root_hash(),
            fee_account: ops_block.fee_account,
            block_transactions: ops,
            processed_priority_ops: (
                last_unprocessed_prior_op,
                self.current_unprocessed_priority_op,
            ),
        };

        self.state.block_number += 1;

        Ok((block, accounts_updated))
    }

    /// Updates the list of accounts that has been updated, aggregates fees, updates blocks operations list from Rollup priority operation
    /// Returns current operation index
    ///
    /// # Arguments
    ///
    /// * `priority_op` - Priority operation
    /// * `op_result` - Rollup priority operation execution result
    /// * `fees` - Rollup operation fees
    /// * `accounts_updated` - Updated accounts
    /// * `current_op_block_index` - Current operation index
    /// * `ops` - Current block operations list
    ///
    fn update_from_priority_operation(
        &mut self,
        priority_op: FranklinPriorityOp,
        op_result: OpSuccess,
        fees: &mut Vec<CollectedFee>,
        accounts_updated: &mut AccountUpdates,
        current_op_block_index: u32,
        ops: &mut Vec<ExecutedOperations>,
    ) -> u32 {
        accounts_updated.append(&mut op_result.updates.clone());
        if let Some(fee) = op_result.fee {
            fees.push(fee);
        }
        let block_index = current_op_block_index;
        let exec_result = ExecutedPriorityOp {
            op: op_result.executed_op,
            priority_op: PriorityOp {
                serial_id: 0,
                data: priority_op,
                deadline_block: 0,
                eth_hash: Vec::new(),
            },
            block_index,
        };
        ops.push(ExecutedOperations::PriorityOp(Box::new(exec_result)));
        self.current_unprocessed_priority_op += 1;
        current_op_block_index + 1
    }

    /// Updates the list of accounts that has been updated, aggregates fees, updates blocks operations list from Rollup transaction
    /// Returns current operation index
    ///
    /// # Arguments
    ///
    /// * `tx` - Rollup transaction
    /// * `op_result` - Rollup transaction execution result
    /// * `fees` - Rollup operation fees
    /// * `accounts_updated` - Updated accounts
    /// * `current_op_block_index` - Current operation index
    /// * `ops` - Current block operations list
    ///
    fn update_from_tx(
        &mut self,
        tx: FranklinTx,
        tx_result: OpSuccess,
        fees: &mut Vec<CollectedFee>,
        accounts_updated: &mut AccountUpdates,
        current_op_block_index: u32,
        ops: &mut Vec<ExecutedOperations>,
    ) -> u32 {
        let OpSuccess {
            fee,
            mut updates,
            executed_op,
        } = tx_result;

        accounts_updated.append(&mut updates);
        if let Some(fee) = fee {
            fees.push(fee);
        }
        let block_index = current_op_block_index;
        let exec_result = ExecutedTx {
            tx,
            success: true,
            op: Some(executed_op),
            fail_reason: None,
            block_index: Some(block_index),
        };
        ops.push(ExecutedOperations::Tx(Box::new(exec_result)));
        current_op_block_index + 1
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
    pub fn get_account_by_address(&self, address: &Address) -> Option<(AccountId, Account)> {
        self.state.get_account_by_address(address)
    }

    /// Returns Franklin Account description by its id
    pub fn get_account(&self, account_id: AccountId) -> Option<Account> {
        self.state.get_account(account_id)
    }
}

#[cfg(test)]
mod test {
    use crate::rollup_ops::RollupOpsBlock;
    use crate::tree_state::TreeState;
    use bigdecimal::BigDecimal;
    use models::node::{
        Deposit, DepositOp, FranklinOp, Transfer, TransferOp, TransferToNewOp, Withdraw, WithdrawOp,
    };

    #[test]
    fn test_update_tree_with_one_tx_per_block() {
        let tx1 = Deposit {
            from: [1u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(1000),
            to: [7u8; 20].into(),
        };
        let op1 = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op: tx1,
            account_id: 0,
        }));
        let pub_data1 = op1.public_data();
        let ops1 =
            RollupOpsBlock::get_rollup_ops_from_data(&pub_data1).expect("cant get ops from data 1");
        let block1 = RollupOpsBlock {
            block_num: 1,
            ops: ops1,
            fee_account: 0,
        };

        let tx2 = Withdraw::new(
            [7u8; 20].into(),
            [7u8; 20].into(),
            1,
            BigDecimal::from(20),
            BigDecimal::from(1),
            1,
            None,
        );
        let op2 = FranklinOp::Withdraw(Box::new(WithdrawOp {
            tx: tx2,
            account_id: 0,
        }));
        let pub_data2 = op2.public_data();
        let ops2 =
            RollupOpsBlock::get_rollup_ops_from_data(&pub_data2).expect("cant get ops from data 2");
        let block2 = RollupOpsBlock {
            block_num: 2,
            ops: ops2,
            fee_account: 0,
        };

        let tx3 = Transfer::new(
            [7u8; 20].into(),
            [8u8; 20].into(),
            1,
            BigDecimal::from(20),
            BigDecimal::from(1),
            3,
            None,
        );
        let op3 = FranklinOp::TransferToNew(Box::new(TransferToNewOp {
            tx: tx3,
            from: 0,
            to: 1,
        }));
        let pub_data3 = op3.public_data();
        let ops3 =
            RollupOpsBlock::get_rollup_ops_from_data(&pub_data3).expect("cant get ops from data 3");
        let block3 = RollupOpsBlock {
            block_num: 3,
            ops: ops3,
            fee_account: 0,
        };

        let tx4 = Transfer::new(
            [8u8; 20].into(),
            [7u8; 20].into(),
            1,
            BigDecimal::from(19),
            BigDecimal::from(1),
            1,
            None,
        );
        let op4 = FranklinOp::Transfer(Box::new(TransferOp {
            tx: tx4,
            from: 1,
            to: 0,
        }));
        let pub_data4 = op4.public_data();
        let ops4 =
            RollupOpsBlock::get_rollup_ops_from_data(&pub_data4).expect("cant get ops from data 4");
        let block4 = RollupOpsBlock {
            block_num: 4,
            ops: ops4,
            fee_account: 0,
        };

        // let tx5 = Close {
        //     account: AccountAddress::from_hex("sync:8888888888888888888888888888888888888888")
        //         .unwrap(),
        //     nonce: 2,
        //     signature: TxSignature::default(),
        // };
        // let op5 = FranklinOp::Close(Box::new(CloseOp {
        //     tx: tx5,
        //     account_id: 1,
        // }));
        // let pub_data5 = op5.public_data();
        // let ops5 =
        //     RollupOpsBlock::get_rollup_ops_from_data(&pub_data5).expect("cant get ops from data 5");
        // let block5 = RollupOpsBlock {
        //     block_num: 5,
        //     ops: ops5,
        //     fee_account: 0,
        // };

        let mut tree = TreeState::new();
        tree.update_tree_states_from_ops_block(&block1)
            .expect("Cant update state from block 1");
        tree.update_tree_states_from_ops_block(&block2)
            .expect("Cant update state from block 2");
        tree.update_tree_states_from_ops_block(&block3)
            .expect("Cant update state from block 3");
        tree.update_tree_states_from_ops_block(&block4)
            .expect("Cant update state from block 4");
        // tree.update_tree_states_from_ops_block(&block5)
        //     .expect("Cant update state from block 5");

        assert_eq!(tree.get_accounts().len(), 2);

        let zero_acc = tree.get_account(0).expect("Cant get 0 account");
        assert_eq!(zero_acc.address, [7u8; 20].into());
        assert_eq!(zero_acc.get_balance(1), BigDecimal::from(980));

        let first_acc = tree.get_account(1).expect("Cant get 0 account");
        assert_eq!(first_acc.address, [8u8; 20].into());
        assert_eq!(first_acc.get_balance(1), BigDecimal::from(0));
    }

    #[test]
    fn test_update_tree_with_multiple_txs_per_block() {
        let tx1 = Deposit {
            from: [1u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(1000),
            to: [7u8; 20].into(),
        };
        let op1 = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op: tx1,
            account_id: 0,
        }));
        let pub_data1 = op1.public_data();

        let tx2 = Withdraw::new(
            [7u8; 20].into(),
            [9u8; 20].into(),
            1,
            BigDecimal::from(20),
            BigDecimal::from(1),
            1,
            None,
        );
        let op2 = FranklinOp::Withdraw(Box::new(WithdrawOp {
            tx: tx2,
            account_id: 0,
        }));
        let pub_data2 = op2.public_data();

        let tx3 = Transfer::new(
            [7u8; 20].into(),
            [8u8; 20].into(),
            1,
            BigDecimal::from(20),
            BigDecimal::from(1),
            3,
            None,
        );
        let op3 = FranklinOp::TransferToNew(Box::new(TransferToNewOp {
            tx: tx3,
            from: 0,
            to: 1,
        }));
        let pub_data3 = op3.public_data();

        let tx4 = Transfer::new(
            [8u8; 20].into(),
            [7u8; 20].into(),
            1,
            BigDecimal::from(19),
            BigDecimal::from(1),
            1,
            None,
        );
        let op4 = FranklinOp::Transfer(Box::new(TransferOp {
            tx: tx4,
            from: 1,
            to: 0,
        }));
        let pub_data4 = op4.public_data();

        let mut pub_data = Vec::new();
        pub_data.extend_from_slice(&pub_data1);
        pub_data.extend_from_slice(&pub_data2);
        pub_data.extend_from_slice(&pub_data3);
        pub_data.extend_from_slice(&pub_data4);

        let ops = RollupOpsBlock::get_rollup_ops_from_data(pub_data.as_slice())
            .expect("cant get ops from data 1");
        let block = RollupOpsBlock {
            block_num: 1,
            ops,
            fee_account: 0,
        };

        let mut tree = TreeState::new();
        tree.update_tree_states_from_ops_block(&block)
            .expect("Cant update state from block");

        assert_eq!(tree.get_accounts().len(), 2);

        let zero_acc = tree.get_account(0).expect("Cant get 0 account");
        assert_eq!(zero_acc.address, [7u8; 20].into());
        assert_eq!(zero_acc.get_balance(1), BigDecimal::from(980));

        let first_acc = tree.get_account(1).expect("Cant get 0 account");
        assert_eq!(first_acc.address, [8u8; 20].into());
        assert_eq!(first_acc.get_balance(1), BigDecimal::from(0));
    }
}
