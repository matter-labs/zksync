use crate::rollup_ops::RollupOpsBlock;
use anyhow::format_err;
use zksync_crypto::Fr;
use zksync_state::{
    handler::TxHandler,
    state::{CollectedFee, OpSuccess, TransferOutcome, ZkSyncState},
};
use zksync_types::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use zksync_types::operations::ZkSyncOp;
use zksync_types::priority_ops::PriorityOp;
use zksync_types::priority_ops::ZkSyncPriorityOp;
use zksync_types::tx::{
    ChangePubKey, Close, ForcedExit, Swap, Transfer, Withdraw, WithdrawNFT, ZkSyncTx,
};
use zksync_types::{account::Account, MintNFT};
use zksync_types::{AccountId, AccountMap, AccountUpdates, Address, BlockNumber, H256};

/// Rollup accounts states
pub struct TreeState {
    /// Accounts stored in a spase merkle tree
    pub state: ZkSyncState,
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
            state: ZkSyncState::empty(),
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
        current_block: BlockNumber,
        accounts: AccountMap,
        current_unprocessed_priority_op: u64,
        fee_account: AccountId,
    ) -> Self {
        let state = ZkSyncState::from_acc_map(accounts, current_block);
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
        available_block_chunk_sizes: &[usize],
    ) -> Result<(Block, AccountUpdates), anyhow::Error> {
        let operations = ops_block.ops.clone();

        let mut accounts_updated = Vec::new();
        let mut fees = Vec::new();
        let mut ops = Vec::new();
        let mut current_op_block_index = 0u32;
        let last_unprocessed_prior_op = self.current_unprocessed_priority_op;

        for operation in operations {
            match operation {
                ZkSyncOp::Deposit(op) => {
                    let priority_op = ZkSyncPriorityOp::Deposit(op.priority_op);
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
                ZkSyncOp::TransferToNew(mut op) => {
                    let from = self
                        .state
                        .get_account(op.from)
                        .ok_or_else(|| format_err!("TransferToNew fail: Nonexistent account"))?;
                    op.tx.from = from.address;
                    op.tx.nonce = from.nonce;
                    let tx = ZkSyncTx::Transfer(Box::new(op.tx.clone()));

                    let raw_op = TransferOutcome::TransferToNew(*op.clone());

                    let (fee, updates) =
                        <ZkSyncState as TxHandler<Transfer>>::apply_op(&mut self.state, &raw_op)
                            .map_err(|e| format_err!("TransferToNew fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::TransferToNew(op),
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
                ZkSyncOp::Transfer(mut op) => {
                    let from = self
                        .state
                        .get_account(op.from)
                        .ok_or_else(|| format_err!("Transfer Fail: Nonexistent account"))?;
                    let to = self
                        .state
                        .get_account(op.to)
                        .ok_or_else(|| format_err!("Transfer Fail: Nonexistent account"))?;
                    op.tx.from = from.address;
                    op.tx.to = to.address;
                    op.tx.nonce = from.nonce;

                    let raw_op = TransferOutcome::Transfer(*op.clone());

                    let tx = ZkSyncTx::Transfer(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<Transfer>>::apply_op(&mut self.state, &raw_op)
                            .map_err(|e| format_err!("Transfer fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::Transfer(op),
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
                ZkSyncOp::Withdraw(mut op) => {
                    // Withdraw op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Withdraw fail: Nonexistent account"))?;
                    op.tx.from = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = ZkSyncTx::Withdraw(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<Withdraw>>::apply_op(&mut self.state, &op)
                            .map_err(|e| format_err!("Withdraw fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::Withdraw(op),
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
                ZkSyncOp::ForcedExit(mut op) => {
                    // Withdraw op comes with empty Account Address and Nonce fields
                    let initiator_account = self
                        .state
                        .get_account(op.tx.initiator_account_id)
                        .ok_or_else(|| {
                            format_err!("ForcedExit fail: Nonexistent initiator account")
                        })?;

                    // Set the fields unknown from the pubdata.
                    op.tx.nonce = initiator_account.nonce;

                    let tx = ZkSyncTx::ForcedExit(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<ForcedExit>>::apply_op(&mut self.state, &op)
                            .map_err(|e| format_err!("ForcedExit fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::ForcedExit(op),
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
                ZkSyncOp::Close(mut op) => {
                    // Close op comes with empty Account Address and Nonce fields
                    let account = self
                        .state
                        .get_account(op.account_id)
                        .ok_or_else(|| format_err!("Close fail: Nonexistent account"))?;
                    op.tx.account = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = ZkSyncTx::Close(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<Close>>::apply_op(&mut self.state, &op)
                            .map_err(|e| format_err!("Close fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::Close(op),
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
                ZkSyncOp::FullExit(op) => {
                    let priority_op = ZkSyncPriorityOp::FullExit(op.priority_op);
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
                ZkSyncOp::ChangePubKeyOffchain(mut op) => {
                    let account = self.state.get_account(op.account_id).ok_or_else(|| {
                        format_err!("ChangePubKeyOffChain fail: Nonexistent account")
                    })?;
                    op.tx.account = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = ZkSyncTx::ChangePubKey(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<ChangePubKey>>::apply_op(&mut self.state, &op)
                            .map_err(|e| format_err!("ChangePubKeyOffChain fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::ChangePubKeyOffchain(op),
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
                ZkSyncOp::Swap(mut op) => {
                    let submitter = self
                        .state
                        .get_account(op.submitter)
                        .ok_or_else(|| format_err!("Swap Fail: Nonexistent account"))?;
                    let account_0 = self
                        .state
                        .get_account(op.accounts.0)
                        .ok_or_else(|| format_err!("Swap Fail: Nonexistent account"))?;
                    let account_1 = self
                        .state
                        .get_account(op.accounts.1)
                        .ok_or_else(|| format_err!("Swap Fail: Nonexistent account"))?;
                    let recipient_0 = self
                        .state
                        .get_account(op.recipients.0)
                        .ok_or_else(|| format_err!("Swap Fail: Nonexistent account"))?;
                    let recipient_1 = self
                        .state
                        .get_account(op.recipients.1)
                        .ok_or_else(|| format_err!("Swap Fail: Nonexistent account"))?;

                    op.tx.submitter_address = submitter.address;
                    op.tx.orders.0.nonce = account_0.nonce;
                    op.tx.orders.0.recipient_address = recipient_0.address;
                    op.tx.orders.1.nonce = account_1.nonce;
                    op.tx.orders.1.recipient_address = recipient_1.address;
                    op.tx.nonce = submitter.nonce;

                    let tx = ZkSyncTx::Swap(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<Swap>>::apply_op(&mut self.state, &op)
                            .map_err(|e| format_err!("Swap fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::Swap(op),
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
                ZkSyncOp::MintNFTOp(mut op) => {
                    let creator = self
                        .state
                        .get_account(op.creator_account_id)
                        .ok_or_else(|| format_err!("MintNFT Fail: Nonexistent creator account"))?;
                    let recipient = self
                        .state
                        .get_account(op.recipient_account_id)
                        .ok_or_else(|| format_err!("MintNFT Fail: Nonexistent recipient"))?;
                    op.tx.creator_address = creator.address;
                    op.tx.recipient = recipient.address;
                    op.tx.nonce = creator.nonce;

                    let tx = ZkSyncTx::MintNFT(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<MintNFT>>::apply_op(&mut self.state, &op)
                            .map_err(|e| format_err!("MintNFT failed: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::MintNFTOp(op),
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
                ZkSyncOp::WithdrawNFT(mut op) => {
                    let account = self
                        .state
                        .get_account(op.tx.account_id)
                        .ok_or_else(|| format_err!("WithdrawNFT fail: Nonexistent account"))?;
                    op.tx.from = account.address;
                    op.tx.nonce = account.nonce;

                    let tx = ZkSyncTx::WithdrawNFT(Box::new(op.tx.clone()));
                    let (fee, updates) =
                        <ZkSyncState as TxHandler<WithdrawNFT>>::apply_op(&mut self.state, &op)
                            .map_err(|e| format_err!("WithdrawNFT fail: {}", e))?;
                    let tx_result = OpSuccess {
                        fee,
                        updates,
                        executed_op: ZkSyncOp::WithdrawNFT(op),
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
                ZkSyncOp::Noop(_) => {}
            }
        }

        let fee_account_address = self
            .get_account(ops_block.fee_account)
            .ok_or_else(|| format_err!("Nonexistent fee account"))?
            .address;

        let fee_updates = self.state.collect_fee(&fees, ops_block.fee_account);
        accounts_updated.extend(fee_updates.into_iter());

        self.last_fee_account_address = fee_account_address;

        // As we restoring an already executed block, this value isn't important.
        let gas_limit = 0.into();

        let block = Block::new_from_available_block_sizes(
            ops_block.block_num,
            self.state.root_hash(),
            ops_block.fee_account,
            ops,
            (
                last_unprocessed_prior_op,
                self.current_unprocessed_priority_op,
            ),
            &available_block_chunk_sizes,
            gas_limit,
            gas_limit,
            ops_block.previous_block_root_hash,
            ops_block.timestamp.unwrap_or_default(),
        );

        *self.state.block_number += 1;

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
        priority_op: ZkSyncPriorityOp,
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
                eth_hash: H256::zero(),
                eth_block: 0,
            },
            block_index,
            created_at: chrono::Utc::now(),
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
        tx: ZkSyncTx,
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
            signed_tx: tx.into(),
            success: true,
            op: Some(executed_op),
            fail_reason: None,
            block_index: Some(block_index),
            created_at: chrono::Utc::now(),
            batch_id: None, // Currently `data_restore` is unable to restore `transaction <--> batch` relation
        };
        ops.push(ExecutedOperations::Tx(Box::new(exec_result)));
        current_op_block_index + 1
    }

    /// Returns map of ZkSync accounts ids and their descriptions
    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.state.get_accounts()
    }

    /// Returns sparse Merkle tree root hash
    pub fn root_hash(&self) -> Fr {
        self.state.root_hash()
    }

    /// Returns ZkSync Account id and description by its address
    pub fn get_account_by_address(&self, address: &Address) -> Option<(AccountId, Account)> {
        self.state.get_account_by_address(address)
    }

    /// Returns ZkSync Account description by its id
    pub fn get_account(&self, account_id: AccountId) -> Option<Account> {
        self.state.get_account(account_id)
    }
}

#[cfg(test)]
mod test {
    use crate::contract::v6::get_rollup_ops_from_data;
    use crate::rollup_ops::RollupOpsBlock;
    use crate::tree_state::TreeState;
    use num::BigUint;
    use zksync_types::tx::ChangePubKey;
    use zksync_types::{
        AccountId, BlockNumber, ChangePubKeyOp, Deposit, DepositOp, ForcedExit, ForcedExitOp,
        FullExit, FullExitOp, Nonce, PubKeyHash, TokenId, Transfer, TransferOp, TransferToNewOp,
        Withdraw, WithdrawOp, ZkSyncOp,
    };

    #[test]
    fn test_update_tree_with_one_tx_per_block() {
        // Deposit 1000 to 7
        let tx1 = Deposit {
            from: [1u8; 20].into(),
            token: TokenId(1),
            amount: BigUint::from(1000u32),
            to: [7u8; 20].into(),
        };
        let op1 = ZkSyncOp::Deposit(Box::new(DepositOp {
            priority_op: tx1,
            account_id: AccountId(0),
        }));
        let pub_data1 = op1.public_data();
        let ops1 = get_rollup_ops_from_data(&pub_data1).expect("cant get ops from data 1");
        let block1 = RollupOpsBlock {
            block_num: BlockNumber(1),
            ops: ops1,
            fee_account: AccountId(0),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };

        // Withdraw 20 with 1 fee from 7 to 10
        let tx2 = Withdraw::new(
            AccountId(0),
            [7u8; 20].into(),
            [9u8; 20].into(),
            TokenId(1),
            BigUint::from(20u32),
            BigUint::from(1u32),
            Nonce(1),
            Default::default(),
            None,
        );
        let op2 = ZkSyncOp::Withdraw(Box::new(WithdrawOp {
            tx: tx2,
            account_id: AccountId(0),
        }));
        let pub_data2 = op2.public_data();
        let ops2 = get_rollup_ops_from_data(&pub_data2).expect("cant get ops from data 2");
        let block2 = RollupOpsBlock {
            block_num: BlockNumber(2),
            ops: ops2,
            fee_account: AccountId(0),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };

        // Transfer 40 with 1 fee from 7 to 8
        let tx3 = Transfer::new(
            AccountId(0),
            [7u8; 20].into(),
            [8u8; 20].into(),
            TokenId(1),
            BigUint::from(40u32),
            BigUint::from(1u32),
            Nonce(3),
            Default::default(),
            None,
        );
        let op3 = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
            tx: tx3,
            from: AccountId(0),
            to: AccountId(1),
        }));
        let pub_data3 = op3.public_data();
        let ops3 = get_rollup_ops_from_data(&pub_data3).expect("cant get ops from data 3");
        let block3 = RollupOpsBlock {
            block_num: BlockNumber(3),
            ops: ops3,
            fee_account: AccountId(0),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };

        // Transfer 19 with 1 fee from 8 to 7
        let tx4 = Transfer::new(
            AccountId(1),
            [8u8; 20].into(),
            [7u8; 20].into(),
            TokenId(1),
            BigUint::from(19u32),
            BigUint::from(1u32),
            Nonce(1),
            Default::default(),
            None,
        );
        let op4 = ZkSyncOp::Transfer(Box::new(TransferOp {
            tx: tx4,
            from: AccountId(1),
            to: AccountId(0),
        }));
        let pub_data4 = op4.public_data();
        let ops4 = get_rollup_ops_from_data(&pub_data4).expect("cant get ops from data 4");
        let block4 = RollupOpsBlock {
            block_num: BlockNumber(4),
            ops: ops4,
            fee_account: AccountId(0),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };

        let pub_key_hash_7 = PubKeyHash::from_hex("sync:8888888888888888888888888888888888888888")
            .expect("Correct pub key hash");
        let tx5 = ChangePubKey::new(
            AccountId(0),
            [7u8; 20].into(),
            pub_key_hash_7,
            TokenId(1),
            BigUint::from(1u32),
            Nonce(2),
            Default::default(),
            None,
            None,
        );
        let op5 = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
            tx: tx5,
            account_id: AccountId(0),
        }));
        let pub_data5 = op5.public_data();
        let ops5 = get_rollup_ops_from_data(&pub_data5).expect("cant get ops from data 5");
        let block5 = RollupOpsBlock {
            block_num: BlockNumber(5),
            ops: ops5,
            fee_account: AccountId(0),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };

        // Full exit for 8
        let tx6 = FullExit {
            account_id: AccountId(1),
            eth_address: [8u8; 20].into(),
            token: TokenId(1),
        };
        let op6 = ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op: tx6,
            withdraw_amount: Some(BigUint::from(980u32).into()),
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        }));
        let pub_data6 = op6.public_data();
        let ops6 = get_rollup_ops_from_data(&pub_data6).expect("cant get ops from data 5");
        let block6 = RollupOpsBlock {
            block_num: BlockNumber(5),
            ops: ops6,
            fee_account: AccountId(0),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };

        // Forced exit for 7
        let tx7 = ForcedExit::new(
            AccountId(0),
            [7u8; 20].into(),
            TokenId(1),
            BigUint::from(1u32),
            Nonce(1),
            Default::default(),
            None,
        );
        let op7 = ZkSyncOp::ForcedExit(Box::new(ForcedExitOp {
            tx: tx7,
            target_account_id: AccountId(0),
            withdraw_amount: Some(BigUint::from(960u32).into()),
        }));
        let pub_data7 = op7.public_data();
        let ops7 = get_rollup_ops_from_data(&pub_data7).expect("cant get ops from data 5");
        let block7 = RollupOpsBlock {
            block_num: BlockNumber(7),
            ops: ops7,
            fee_account: AccountId(1),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };
        // This transaction have to be deleted, do not uncomment. Delete it after removing the corresponding code        // let tx6 = Close {
        //     account: Address::from_hex("sync:8888888888888888888888888888888888888888").unwrap(),
        //     nonce: 2,
        //     signature: TxSignature::default(),
        // };
        // let op6 = ZkSyncOp::Close(Box::new(CloseOp {
        //     tx: tx6,
        //     account_id: 1,
        // }));
        // let pub_data6 = op6.public_data();
        // let ops6 =
        //     RollupOpsBlock::get_rollup_ops_from_data(&pub_data5).expect("cant get ops from data 5");
        // let block5 = RollupOpsBlock {
        //     block_num: 6,
        //     ops: ops6,
        //     fee_account: AccountId(0),
        // };
        //
        let available_block_chunk_sizes = vec![10, 32, 72, 156, 322, 654];
        let mut tree = TreeState::new();
        tree.update_tree_states_from_ops_block(&block1, &available_block_chunk_sizes)
            .expect("Cant update state from block 1");
        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        assert_eq!(zero_acc.address, [7u8; 20].into());
        assert_eq!(zero_acc.get_balance(TokenId(1)), BigUint::from(1000u32));

        tree.update_tree_states_from_ops_block(&block2, &available_block_chunk_sizes)
            .expect("Cant update state from block 2");
        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        assert_eq!(zero_acc.get_balance(TokenId(1)), BigUint::from(980u32));

        tree.update_tree_states_from_ops_block(&block3, &available_block_chunk_sizes)
            .expect("Cant update state from block 3");
        // Verify creating accounts
        assert_eq!(tree.get_accounts().len(), 2);

        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        let first_acc = tree.get_account(AccountId(1)).expect("Cant get 0 account");
        assert_eq!(first_acc.address, [8u8; 20].into());

        assert_eq!(zero_acc.get_balance(TokenId(1)), BigUint::from(940u32));
        assert_eq!(first_acc.get_balance(TokenId(1)), BigUint::from(40u32));

        tree.update_tree_states_from_ops_block(&block4, &available_block_chunk_sizes)
            .expect("Cant update state from block 4");
        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        let first_acc = tree.get_account(AccountId(1)).expect("Cant get 0 account");
        assert_eq!(zero_acc.get_balance(TokenId(1)), BigUint::from(960u32));
        assert_eq!(first_acc.get_balance(TokenId(1)), BigUint::from(20u32));

        assert_eq!(zero_acc.pub_key_hash, PubKeyHash::zero());
        tree.update_tree_states_from_ops_block(&block5, &available_block_chunk_sizes)
            .expect("Cant update state from block 5");
        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        assert_eq!(zero_acc.pub_key_hash, pub_key_hash_7);

        tree.update_tree_states_from_ops_block(&block6, &available_block_chunk_sizes)
            .expect("Cant update state from block 6");
        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        let first_acc = tree.get_account(AccountId(1)).expect("Cant get 0 account");
        assert_eq!(zero_acc.get_balance(TokenId(1)), BigUint::from(960u32));
        assert_eq!(first_acc.get_balance(TokenId(1)), BigUint::from(0u32));

        tree.update_tree_states_from_ops_block(&block7, &available_block_chunk_sizes)
            .expect("Cant update state from block 7");
        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        let first_acc = tree.get_account(AccountId(1)).expect("Cant get 0 account");
        assert_eq!(zero_acc.get_balance(TokenId(1)), BigUint::from(0u32));
        assert_eq!(first_acc.get_balance(TokenId(1)), BigUint::from(1u32));
    }

    #[test]
    fn test_update_tree_with_multiple_txs_per_block() {
        let tx1 = Deposit {
            from: [1u8; 20].into(),
            token: TokenId(1),
            amount: BigUint::from(1000u32),
            to: [7u8; 20].into(),
        };
        let op1 = ZkSyncOp::Deposit(Box::new(DepositOp {
            priority_op: tx1,
            account_id: AccountId(0),
        }));
        let pub_data1 = op1.public_data();

        let tx2 = Withdraw::new(
            AccountId(0),
            [7u8; 20].into(),
            [9u8; 20].into(),
            TokenId(1),
            BigUint::from(20u32),
            BigUint::from(1u32),
            Nonce(1),
            Default::default(),
            None,
        );
        let op2 = ZkSyncOp::Withdraw(Box::new(WithdrawOp {
            tx: tx2,
            account_id: AccountId(0),
        }));
        let pub_data2 = op2.public_data();

        let tx3 = Transfer::new(
            AccountId(0),
            [7u8; 20].into(),
            [8u8; 20].into(),
            TokenId(1),
            BigUint::from(40u32),
            BigUint::from(1u32),
            Nonce(3),
            Default::default(),
            None,
        );
        let op3 = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
            tx: tx3,
            from: AccountId(0),
            to: AccountId(1),
        }));
        let pub_data3 = op3.public_data();

        let tx4 = Transfer::new(
            AccountId(1),
            [8u8; 20].into(),
            [7u8; 20].into(),
            TokenId(1),
            BigUint::from(19u32),
            BigUint::from(1u32),
            Nonce(1),
            Default::default(),
            None,
        );
        let op4 = ZkSyncOp::Transfer(Box::new(TransferOp {
            tx: tx4,
            from: AccountId(1),
            to: AccountId(0),
        }));
        let pub_data4 = op4.public_data();

        let pub_key_hash_7 = PubKeyHash::from_hex("sync:8888888888888888888888888888888888888888")
            .expect("Correct pub key hash");
        let tx5 = ChangePubKey::new(
            AccountId(0),
            [7u8; 20].into(),
            pub_key_hash_7,
            TokenId(1),
            BigUint::from(1u32),
            Nonce(2),
            Default::default(),
            None,
            None,
        );
        let op5 = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
            tx: tx5,
            account_id: AccountId(0),
        }));
        let pub_data5 = op5.public_data();

        let tx6 = FullExit {
            account_id: AccountId(1),
            eth_address: [8u8; 20].into(),
            token: TokenId(1),
        };
        let op6 = ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op: tx6,
            withdraw_amount: Some(BigUint::from(980u32).into()),
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        }));
        let pub_data6 = op6.public_data();

        let tx7 = ForcedExit::new(
            AccountId(0),
            [7u8; 20].into(),
            TokenId(1),
            BigUint::from(1u32),
            Nonce(1),
            Default::default(),
            None,
        );
        let op7 = ZkSyncOp::ForcedExit(Box::new(ForcedExitOp {
            tx: tx7,
            target_account_id: AccountId(0),
            withdraw_amount: Some(BigUint::from(956u32).into()),
        }));
        let pub_data7 = op7.public_data();

        let mut pub_data = Vec::new();
        pub_data.extend_from_slice(&pub_data1);
        pub_data.extend_from_slice(&pub_data2);
        pub_data.extend_from_slice(&pub_data3);
        pub_data.extend_from_slice(&pub_data4);
        pub_data.extend_from_slice(&pub_data5);
        pub_data.extend_from_slice(&pub_data6);
        pub_data.extend_from_slice(&pub_data7);

        let ops = get_rollup_ops_from_data(pub_data.as_slice()).expect("cant get ops from data 1");
        let block = RollupOpsBlock {
            block_num: BlockNumber(1),
            ops,
            fee_account: AccountId(0),
            timestamp: None,
            previous_block_root_hash: Default::default(),
            contract_version: None,
        };

        let mut tree = TreeState::new();
        let available_block_chunk_sizes = vec![10, 32, 72, 156, 322, 654];
        tree.update_tree_states_from_ops_block(&block, &available_block_chunk_sizes)
            .expect("Cant update state from block");

        assert_eq!(tree.get_accounts().len(), 2);

        let zero_acc = tree.get_account(AccountId(0)).expect("Cant get 0 account");
        assert_eq!(zero_acc.address, [7u8; 20].into());
        assert_eq!(zero_acc.get_balance(TokenId(1)), BigUint::from(5u32));
        assert_eq!(zero_acc.pub_key_hash, pub_key_hash_7);

        let first_acc = tree.get_account(AccountId(1)).expect("Cant get 0 account");
        assert_eq!(first_acc.address, [8u8; 20].into());
        assert_eq!(first_acc.get_balance(TokenId(1)), BigUint::from(0u32));
    }
}
