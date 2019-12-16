use crate::franklin_ops::FranklinOpsBlock;
use failure::format_err;
use models::node::account::{Account, AccountAddress};
use models::node::operations::FranklinOp;
use models::node::priority_ops::FranklinPriorityOp;
use models::node::tx::FranklinTx;
use models::CommitRequest;
use models::node::priority_ops::PriorityOp;
use models::node::{AccountId, AccountMap, AccountUpdates, Fr};
use plasma::state::{OpSuccess, PlasmaState, CollectedFee};
use bigdecimal::BigDecimal;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};

/// Franklin Accounts states with data restore configuration
pub struct FranklinAccountsState {
    /// Accounts stored in a spase merkle tree
    pub state: PlasmaState,
    pub current_unprocessed_priority_op: u64,
    pub last_fee_account_address: AccountAddress
}

impl Default for FranklinAccountsState {
    fn default() -> Self {
        Self::new()
    }
}

impl FranklinAccountsState {
    /// Returns new FranklinAccountsState instance
    pub fn new() -> Self {
        Self {
            state: PlasmaState::empty(),
            current_unprocessed_priority_op: 0,
            last_fee_account_address: AccountAddress::default()
        }
    }

    /// Returns FranklinAccountsState from accounts map and current block number
    ///
    /// # Arguments
    ///
    /// * `current_block` - current block number
    /// * `accounts` - accounts map
    ///
    pub fn load(
        current_block: u32,
        accounts: AccountMap,
        fee_account: AccountAddress,
        current_unprocessed_priority_op: u64
    ) -> Self {
        Self {
            state: PlasmaState::new(accounts, current_block),
            current_unprocessed_priority_op: current_unprocessed_priority_op,
            last_fee_account_address: fee_account
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
        ops_block: &FranklinOpsBlock,
    ) -> Result<CommitRequest, failure::Error> {
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
                    let op_result = self
                        .state
                        .execute_priority_op(priority_op.clone());
                    current_op_block_index = self.update_from_priority_operation(
                        priority_op,
                        op_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops
                    );
                }
                FranklinOp::TransferToNew(mut op) => {
                    let from = self
                        .state
                        .get_account(op.from)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    op.tx.from = from.address;
                    op.tx.nonce = from.nonce;

                    let tx = FranklinTx::Transfer(op.tx);
                    let tx_result = self.state.execute_tx(tx.clone());
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops
                    );
                }
                FranklinOp::Withdraw(mut op) => {
                    // Withdraw op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    op.tx.account = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = FranklinTx::Withdraw(op.tx);
                    let tx_result = self.state.execute_tx(tx.clone());
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops
                    );
                }
                FranklinOp::Close(mut op) => {
                    // Close op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    op.tx.account = account.address;
                    op.tx.nonce = account.nonce;
                    
                    let tx = FranklinTx::Close(op.tx);
                    let tx_result = self.state.execute_tx(tx.clone());
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops
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

                    let tx = FranklinTx::Transfer(op.tx);
                    let tx_result = self.state.execute_tx(tx.clone());
                    current_op_block_index = self.update_from_tx(
                        tx,
                        tx_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops
                    );
                }
                FranklinOp::FullExit(mut op) => {
                    op.priority_op.nonce -= 1;
                    let priority_op = FranklinPriorityOp::FullExit(op.priority_op);
                    let op_result = self
                        .state
                        .execute_priority_op(priority_op.clone());
                    current_op_block_index = self.update_from_priority_operation(
                        priority_op,
                        op_result,
                        &mut fees,
                        &mut accounts_updated,
                        current_op_block_index,
                        &mut ops
                    );
                }
                _ => {}
            }
        }
        let fee_account_address = self
            .get_account(ops_block.fee_account)
            .ok_or_else(|| format_err!("Nonexistent account"))?
            .address;
        let (fee_account_id, fee_updates) = self.state.collect_fee(&fees, &fee_account_address);
        accounts_updated.extend(fee_updates.into_iter());

        self.last_fee_account_address = fee_account_address;

        let block = Block {
            block_number: ops_block.block_num,
            new_root_hash: self.state.root_hash(),
            fee_account: fee_account_id,
            block_transactions: ops,
            processed_priority_ops: (
                last_unprocessed_prior_op,
                self.current_unprocessed_priority_op,
            ),
        };

        self.state.block_number += 1;

        Ok(CommitRequest {
            block,
            accounts_updated,
        })
    }

    fn update_from_priority_operation(
        &mut self,
        priority_op: FranklinPriorityOp,
        op_result: OpSuccess,
        fees: &mut Vec<CollectedFee>,
        accounts_updated: &mut AccountUpdates,
        current_op_block_index: u32,
        ops: &mut Vec<ExecutedOperations>
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
                eth_fee: BigDecimal::from(0),
                eth_hash: Vec::new(),
            },
            block_index,
        };
        ops.push(ExecutedOperations::PriorityOp(Box::new(exec_result)));
        self.current_unprocessed_priority_op += 1;
        current_op_block_index + 1
    }

    fn update_from_tx(
        &mut self,
        tx: FranklinTx,
        tx_result: Result<OpSuccess, failure::Error>,
        fees: &mut Vec<CollectedFee>,
        accounts_updated: &mut AccountUpdates,
        current_op_block_index: u32,
        ops: &mut Vec<ExecutedOperations>
    ) -> u32 {
        match tx_result {
            Ok(OpSuccess {
                fee,
                mut updates,
                executed_op,
            }) => {
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
            },
            Err(e) => {
                let exec_result = ExecutedTx {
                    tx,
                    success: false,
                    op: None,
                    fail_reason: Some(e.to_string()),
                    block_index: None,
                };
                ops.push(ExecutedOperations::Tx(Box::new(exec_result)));
                current_op_block_index
            }
        }
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
    use crate::accounts_state::FranklinAccountsState;
    use crate::franklin_ops::FranklinOpsBlock;
    use bigdecimal::BigDecimal;
    use models::node::tx::TxSignature;
    use models::node::{
        AccountAddress, Close, CloseOp, Deposit, DepositOp, FranklinOp, Transfer, TransferOp,
        TransferToNewOp, Withdraw, WithdrawOp,
    };

    #[test]
    fn test_update_tree_with_one_tx_per_block() {
        let tx1 = Deposit {
            sender: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(1000),
            account: AccountAddress::from_hex("0x7777777777777777777777777777777777777777")
                .unwrap(),
        };
        let op1 = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op: tx1,
            account_id: 0,
        }));
        let pub_data1 = op1.public_data();
        let ops1 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data1)
            .expect("cant get ops from data 1");
        let block1 = FranklinOpsBlock {
            block_num: 1,
            ops: ops1,
            fee_account: 0,
        };

        let tx2 = Withdraw {
            account: AccountAddress::from_hex("0x7777777777777777777777777777777777777777")
                .unwrap(),
            eth_address: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(20),
            fee: BigDecimal::from(1),
            nonce: 1,
            signature: TxSignature::default(),
        };
        let op2 = FranklinOp::Withdraw(Box::new(WithdrawOp {
            tx: tx2,
            account_id: 0,
        }));
        let pub_data2 = op2.public_data();
        let ops2 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data2)
            .expect("cant get ops from data 2");
        let block2 = FranklinOpsBlock {
            block_num: 2,
            ops: ops2,
            fee_account: 0,
        };

        let tx3 = Transfer {
            from: AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap(),
            to: AccountAddress::from_hex("0x8888888888888888888888888888888888888888").unwrap(),
            token: 1,
            amount: BigDecimal::from(20),
            fee: BigDecimal::from(1),
            nonce: 3,
            signature: TxSignature::default(),
        };
        let op3 = FranklinOp::TransferToNew(Box::new(TransferToNewOp {
            tx: tx3,
            from: 0,
            to: 1,
        }));
        let pub_data3 = op3.public_data();
        let ops3 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data3)
            .expect("cant get ops from data 3");
        let block3 = FranklinOpsBlock {
            block_num: 3,
            ops: ops3,
            fee_account: 0,
        };

        let tx4 = Transfer {
            from: AccountAddress::from_hex("0x8888888888888888888888888888888888888888").unwrap(),
            to: AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap(),
            token: 1,
            amount: BigDecimal::from(19),
            fee: BigDecimal::from(1),
            nonce: 1,
            signature: TxSignature::default(),
        };
        let op4 = FranklinOp::Transfer(Box::new(TransferOp {
            tx: tx4,
            from: 1,
            to: 0,
        }));
        let pub_data4 = op4.public_data();
        let ops4 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data4)
            .expect("cant get ops from data 4");
        let block4 = FranklinOpsBlock {
            block_num: 4,
            ops: ops4,
            fee_account: 0,
        };

        let tx5 = Close {
            account: AccountAddress::from_hex("0x8888888888888888888888888888888888888888")
                .unwrap(),
            nonce: 2,
            signature: TxSignature::default(),
        };
        let op5 = FranklinOp::Close(Box::new(CloseOp {
            tx: tx5,
            account_id: 1,
        }));
        let pub_data5 = op5.public_data();
        let ops5 = FranklinOpsBlock::get_franklin_ops_from_data(&pub_data5)
            .expect("cant get ops from data 5");
        let block5 = FranklinOpsBlock {
            block_num: 5,
            ops: ops5,
            fee_account: 0,
        };

        let mut tree = FranklinAccountsState::new();
        tree.update_accounts_states_from_ops_block(&block1)
            .expect("Cant update state from block 1");
        tree.update_accounts_states_from_ops_block(&block2)
            .expect("Cant update state from block 2");
        tree.update_accounts_states_from_ops_block(&block3)
            .expect("Cant update state from block 3");
        tree.update_accounts_states_from_ops_block(&block4)
            .expect("Cant update state from block 4");
        tree.update_accounts_states_from_ops_block(&block5)
            .expect("Cant update state from block 5");

        assert_eq!(tree.get_accounts().len(), 2);

        let zero_acc = tree.get_account(0).expect("Cant get 0 account");
        assert_eq!(
            zero_acc.address,
            AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap()
        );
        assert_eq!(zero_acc.get_balance(1), BigDecimal::from(980));

        let first_acc = tree.get_account(1).expect("Cant get 0 account");
        assert_eq!(
            first_acc.address,
            AccountAddress::from_hex("0x0000000000000000000000000000000000000000").unwrap()
        );
        assert_eq!(first_acc.get_balance(1), BigDecimal::from(0));
    }

    #[test]
    fn test_update_tree_with_multiple_txs_per_block() {
        let tx1 = Deposit {
            sender: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(1000),
            account: AccountAddress::from_hex("0x7777777777777777777777777777777777777777")
                .unwrap(),
        };
        let op1 = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op: tx1,
            account_id: 0,
        }));
        let pub_data1 = op1.public_data();

        let tx2 = Withdraw {
            account: AccountAddress::from_hex("0x7777777777777777777777777777777777777777")
                .unwrap(),
            eth_address: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(20),
            fee: BigDecimal::from(1),
            nonce: 1,
            signature: TxSignature::default(),
        };
        let op2 = FranklinOp::Withdraw(Box::new(WithdrawOp {
            tx: tx2,
            account_id: 0,
        }));
        let pub_data2 = op2.public_data();

        let tx3 = Transfer {
            from: AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap(),
            to: AccountAddress::from_hex("0x8888888888888888888888888888888888888888").unwrap(),
            token: 1,
            amount: BigDecimal::from(20),
            fee: BigDecimal::from(1),
            nonce: 3,
            signature: TxSignature::default(),
        };
        let op3 = FranklinOp::TransferToNew(Box::new(TransferToNewOp {
            tx: tx3,
            from: 0,
            to: 1,
        }));
        let pub_data3 = op3.public_data();

        let tx4 = Transfer {
            from: AccountAddress::from_hex("0x8888888888888888888888888888888888888888").unwrap(),
            to: AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap(),
            token: 1,
            amount: BigDecimal::from(19),
            fee: BigDecimal::from(1),
            nonce: 1,
            signature: TxSignature::default(),
        };
        let op4 = FranklinOp::Transfer(Box::new(TransferOp {
            tx: tx4,
            from: 1,
            to: 0,
        }));
        let pub_data4 = op4.public_data();

        let tx5 = Close {
            account: AccountAddress::from_hex("0x8888888888888888888888888888888888888888")
                .unwrap(),
            nonce: 2,
            signature: TxSignature::default(),
        };
        let op5 = FranklinOp::Close(Box::new(CloseOp {
            tx: tx5,
            account_id: 1,
        }));
        let pub_data5 = op5.public_data();

        let mut pub_data = Vec::new();
        pub_data.extend_from_slice(&pub_data1);
        pub_data.extend_from_slice(&pub_data2);
        pub_data.extend_from_slice(&pub_data3);
        pub_data.extend_from_slice(&pub_data4);
        pub_data.extend_from_slice(&pub_data5);

        let ops = FranklinOpsBlock::get_franklin_ops_from_data(pub_data.as_slice())
            .expect("cant get ops from data 1");
        let block = FranklinOpsBlock {
            block_num: 1,
            ops,
            fee_account: 0,
        };

        let mut tree = FranklinAccountsState::new();
        tree.update_accounts_states_from_ops_block(&block)
            .expect("Cant update state from block");

        assert_eq!(tree.get_accounts().len(), 2);

        let zero_acc = tree.get_account(0).expect("Cant get 0 account");
        assert_eq!(
            zero_acc.address,
            AccountAddress::from_hex("0x7777777777777777777777777777777777777777").unwrap()
        );
        assert_eq!(zero_acc.get_balance(1), BigDecimal::from(980));

        let first_acc = tree.get_account(1).expect("Cant get 0 account");
        assert_eq!(
            first_acc.address,
            AccountAddress::from_hex("0x0000000000000000000000000000000000000000").unwrap()
        );
        assert_eq!(first_acc.get_balance(1), BigDecimal::from(0));
    }
}
