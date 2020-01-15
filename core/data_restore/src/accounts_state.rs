use crate::franklin_ops::FranklinOpsBlock;
use failure::format_err;
use models::node::account::{Account, PubKeyHash};
use models::node::operations::FranklinOp;
use models::node::priority_ops::FranklinPriorityOp;
use models::node::tx::FranklinTx;
use models::node::{AccountId, AccountMap, AccountUpdates, Fr};
use plasma::state::{OpSuccess, PlasmaState};
use web3::types::Address;

/// Franklin Accounts states with data restore configuration
pub struct FranklinAccountsState {
    /// Accounts stored in a spase merkle tree
    pub state: PlasmaState,
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
    ) -> Result<AccountUpdates, failure::Error> {
        let operations = block.ops.clone();

        let mut accounts_updated = Vec::new();
        let mut fees = Vec::new();

        for operation in operations {
            match operation {
                FranklinOp::Deposit(op) => {
                    let OpSuccess {
                        fee, mut updates, ..
                    } = self
                        .state
                        .execute_priority_op(FranklinPriorityOp::Deposit(op.priority_op));
                    if let Some(fee) = fee {
                        fees.push(fee);
                    }
                    accounts_updated.append(&mut updates);
                }
                FranklinOp::TransferToNew(mut op) => {
                    let from = self
                        .state
                        .get_account(op.from)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    op.tx.from = from.address;
                    op.tx.nonce = from.nonce;
                    if let Ok(OpSuccess {
                        fee, mut updates, ..
                    }) = self.state.execute_tx(FranklinTx::Transfer(op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                }
                FranklinOp::Withdraw(mut op) => {
                    // Withdraw op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    op.tx.from = account.address;
                    op.tx.nonce = account.nonce;
                    if let Ok(OpSuccess {
                        fee, mut updates, ..
                    }) = self.state.execute_tx(FranklinTx::Withdraw(op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                }
                FranklinOp::Close(mut op) => {
                    // Close op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Nonexistent account"))?;
                    op.tx.account = account.address;
                    op.tx.nonce = account.nonce;
                    if let Ok(OpSuccess {
                        fee, mut updates, ..
                    }) = self.state.execute_tx(FranklinTx::Close(op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
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
                    if let Ok(OpSuccess {
                        fee, mut updates, ..
                    }) = self.state.execute_tx(FranklinTx::Transfer(op.tx))
                    {
                        if let Some(fee) = fee {
                            fees.push(fee);
                        }
                        accounts_updated.append(&mut updates);
                    }
                }
                FranklinOp::FullExit(mut op) => {
                    let OpSuccess {
                        fee, mut updates, ..
                    } = self
                        .state
                        .execute_priority_op(FranklinPriorityOp::FullExit(op.priority_op));
                    if let Some(fee) = fee {
                        fees.push(fee);
                    }
                    accounts_updated.append(&mut updates);
                }
                _ => {}
            }
        }
        let fee_updates = self.state.collect_fee(&fees, block.fee_account);
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
    use crate::accounts_state::FranklinAccountsState;
    use crate::franklin_ops::FranklinOpsBlock;
    use bigdecimal::BigDecimal;
    use models::node::tx::TxSignature;
    use models::node::{
        Close, CloseOp, Deposit, DepositOp, FranklinOp, PubKeyHash, Transfer, TransferOp,
        TransferToNewOp, Withdraw, WithdrawOp,
    };

    #[test]
    fn test_update_tree_with_one_tx_per_block() {
        let tx1 = Deposit {
            from: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(1000),
            to: "0x7777777777777777777777777777777777777777"
                .parse()
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
            from: "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap(),
            to: [9u8; 20].into(),
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
            from: "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap(),
            to: "0x8888888888888888888888888888888888888888"
                .parse()
                .unwrap(),
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
            from: "0x8888888888888888888888888888888888888888"
                .parse()
                .unwrap(),
            to: "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap(),
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
            account: "0x8888888888888888888888888888888888888888"
                .parse()
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
            "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap()
        );
        assert_eq!(zero_acc.get_balance(1), BigDecimal::from(980));

        let first_acc = tree.get_account(1).expect("Cant get 0 account");
        assert_eq!(
            first_acc.address,
            "0x0000000000000000000000000000000000000000"
                .parse()
                .unwrap()
        );
        assert_eq!(first_acc.get_balance(1), BigDecimal::from(0));
    }

    #[test]
    fn test_update_tree_with_multiple_txs_per_block() {
        let tx1 = Deposit {
            from: [9u8; 20].into(),
            token: 1,
            amount: BigDecimal::from(1000),
            to: "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap(),
        };
        let op1 = FranklinOp::Deposit(Box::new(DepositOp {
            priority_op: tx1,
            account_id: 0,
        }));
        let pub_data1 = op1.public_data();

        let tx2 = Withdraw {
            from: "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap(),
            to: [9u8; 20].into(),
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
            from: "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap(),
            to: "0x8888888888888888888888888888888888888888"
                .parse()
                .unwrap(),
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
            from: "0x8888888888888888888888888888888888888888"
                .parse()
                .unwrap(),
            to: "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap(),
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
            account: "0x8888888888888888888888888888888888888888"
                .parse()
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
            "0x7777777777777777777777777777777777777777"
                .parse()
                .unwrap()
        );
        assert_eq!(zero_acc.get_balance(1), BigDecimal::from(980));

        let first_acc = tree.get_account(1).expect("Cant get 0 account");
        assert_eq!(
            first_acc.address,
            "0x0000000000000000000000000000000000000000"
                .parse()
                .unwrap()
        );
        assert_eq!(first_acc.get_balance(1), BigDecimal::from(0));
    }
}
