// Built-in imports
// External imports
use chrono::{DateTime, Duration, Utc};
use num::BigUint;
// Workspace imports
use zksync_basic_types::H256;
use zksync_crypto::{franklin_crypto::bellman::pairing::ff::Field, Fr};
use zksync_test_account::ZkSyncAccount;
use zksync_types::{
    block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx},
    operations::{ChangePubKeyOp, ZkSyncOp},
    priority_ops::PriorityOp,
    tx::{ChangePubKeyType, TxHash},
    AccountId, Address, BlockNumber, CloseOp, Deposit, DepositOp, FullExit, FullExitOp, Token,
    TokenId, TransferOp, TransferToNewOp, WithdrawOp,
};
// Local imports

pub struct TransactionsHistoryTestSetup {
    pub from_zksync_account: ZkSyncAccount,
    pub to_zksync_account: ZkSyncAccount,

    pub amount: BigUint,

    pub tokens: Vec<Token>,
    pub blocks: Vec<Block>,

    pub next_tx_time: DateTime<Utc>,
}

impl TransactionsHistoryTestSetup {
    pub fn new() -> Self {
        let tokens = vec![
            Token::new(TokenId(0), Address::zero(), "ETH", 18), // used for deposits
            Token::new(TokenId(1), Address::random(), "DAI", 18), // used for transfers
            Token::new(TokenId(2), Address::random(), "FAU", 6), // used for withdraws
        ];

        let from_account_id = AccountId(0xbabe);
        let from_zksync_account = ZkSyncAccount::rand();
        from_zksync_account.set_account_id(Some(from_account_id));

        let to_account_id = AccountId(0xdcba);
        let to_zksync_account = ZkSyncAccount::rand();
        to_zksync_account.set_account_id(Some(to_account_id));

        let amount = 1u32.into();

        Self {
            from_zksync_account,
            to_zksync_account,

            amount,

            tokens,
            blocks: Vec::new(),

            next_tx_time: Utc::now(),
        }
    }

    pub fn get_tx_hash(&self, block_number: usize, block_index: usize) -> TxHash {
        match &self.blocks[block_number].block_transactions[block_index] {
            ExecutedOperations::PriorityOp(op) => op.priority_op.tx_hash(),
            ExecutedOperations::Tx(tx) => tx.signed_tx.hash(),
        }
    }

    pub fn add_block(&mut self, block_id: u32) {
        let prior_op_unique_serial_id = u64::from(block_id * 2);
        let executed_deposit_op = self.create_deposit_op(prior_op_unique_serial_id, block_id, 0);
        let executed_transfer_to_new_op = self.create_transfer_to_new_op(Some(1));
        let executed_transfer_op = self.create_transfer_tx(Some(2));
        let executed_close_op = self.create_close_tx(Some(3));
        let executed_change_pubkey_op = self.create_change_pubkey_tx(Some(4));
        let executed_withdraw_op = self.create_withdraw_tx(Some(5));
        let executed_full_exit_op =
            self.create_full_exit_op(prior_op_unique_serial_id + 1, block_id, 6);

        let operations = vec![
            executed_deposit_op,
            executed_transfer_to_new_op,
            executed_transfer_op,
            executed_close_op,
            executed_change_pubkey_op,
            executed_withdraw_op,
            executed_full_exit_op,
        ];

        let block = Block::new(
            BlockNumber(block_id),
            Fr::zero(),
            AccountId(0),
            operations,
            (0, 0), // Not important
            100,
            1_000_000.into(), // Not important
            1_500_000.into(), // Not important
            H256::default(),
            0,
        );

        self.blocks.push(block);
    }

    pub fn add_block_with_rejected_op(&mut self, block_id: u32) {
        let prior_op_unique_serial_id = u64::from(block_id * 2);
        let executed_deposit_op = self.create_deposit_op(prior_op_unique_serial_id, block_id, 0);
        let executed_transfer_to_new_op = self.create_transfer_to_new_op(Some(1));
        let rejected_transfer_op = self.create_transfer_tx(None);
        let executed_close_op = self.create_close_tx(Some(2));
        let executed_change_pubkey_op = self.create_change_pubkey_tx(Some(3));
        let executed_withdraw_op = self.create_withdraw_tx(Some(4));
        let executed_full_exit_op =
            self.create_full_exit_op(prior_op_unique_serial_id + 1, block_id, 6);

        let operations = vec![
            executed_deposit_op,
            executed_transfer_to_new_op,
            rejected_transfer_op,
            executed_close_op,
            executed_change_pubkey_op,
            executed_withdraw_op,
            executed_full_exit_op,
        ];

        let block = Block::new(
            BlockNumber(block_id),
            Fr::zero(),
            AccountId(0),
            operations,
            (0, 0), // Not important
            100,
            1_000_000.into(), // Not important
            1_500_000.into(), // Not important
            Default::default(),
            0,
        );

        self.blocks.push(block);
    }

    pub fn add_block_with_batch(&mut self, block_id: u32, success: bool) {
        let block_indexes = if success {
            vec![Some(0), Some(1), Some(2)]
        } else {
            vec![None, None, None]
        };
        let transfer_op_0 = self.create_transfer_tx(block_indexes[0]);
        let transfer_op_1 = self.create_transfer_tx(block_indexes[1]);
        let transfer_op_2 = self.create_transfer_tx(block_indexes[2]);

        let operations = vec![transfer_op_0, transfer_op_1, transfer_op_2];

        let block = Block::new(
            BlockNumber(block_id),
            Fr::zero(),
            AccountId(0),
            operations,
            (0, 0), // Not important
            100,
            1_000_000.into(), // Not important
            1_500_000.into(), // Not important
            Default::default(),
            0,
        );

        self.blocks.push(block);
    }

    fn create_deposit_op(
        &mut self,
        serial_id: u64,
        block: u32,
        block_index: u32,
    ) -> ExecutedOperations {
        let deposit_op = ZkSyncOp::Deposit(Box::new(DepositOp {
            priority_op: Deposit {
                from: self.from_zksync_account.address,
                token: self.tokens[0].id,
                amount: self.amount.clone(),
                to: self.to_zksync_account.address,
            },
            account_id: self.from_zksync_account.get_account_id().unwrap(),
        }));

        let executed_op = ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id,
                data: deposit_op.try_get_priority_op().unwrap(),
                deadline_block: 0,
                eth_hash: H256::from_slice(
                    &hex::decode(format!("{:0>64}", format!("{}{}", block, block_index))).unwrap(),
                ),
                eth_block: 10,
                eth_block_index: Some(1),
            },
            op: deposit_op,
            block_index,
            created_at: self.get_tx_time(),
        };

        ExecutedOperations::PriorityOp(Box::new(executed_op))
    }

    fn create_full_exit_op(
        &mut self,
        serial_id: u64,
        block: u32,
        block_index: u32,
    ) -> ExecutedOperations {
        let full_exit_op = ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op: FullExit {
                account_id: self.from_zksync_account.get_account_id().unwrap(),
                eth_address: self.from_zksync_account.address,
                token: self.tokens[2].id,
                is_legacy: false,
            },
            withdraw_amount: Some(self.amount.clone().into()),
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        }));

        let executed_op = ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id,
                data: full_exit_op.try_get_priority_op().unwrap(),
                deadline_block: 0,
                eth_hash: H256::from_slice(
                    &hex::decode(format!("{:0>64}", format!("{}{}", block, block_index))).unwrap(),
                ),
                eth_block: 11,
                eth_block_index: Some(1),
            },
            op: full_exit_op,
            block_index,
            created_at: self.get_tx_time(),
        };

        ExecutedOperations::PriorityOp(Box::new(executed_op))
    }

    fn create_transfer_to_new_op(&mut self, block_index: Option<u32>) -> ExecutedOperations {
        let transfer_to_new_op = ZkSyncOp::TransferToNew(Box::new(TransferToNewOp {
            tx: self
                .from_zksync_account
                .sign_transfer(
                    self.tokens[1].id,
                    &self.tokens[1].symbol,
                    self.amount.clone(),
                    0u32.into(),
                    &self.to_zksync_account.address,
                    None,
                    true,
                    Default::default(),
                )
                .0,
            from: self.from_zksync_account.get_account_id().unwrap(),
            to: self.to_zksync_account.get_account_id().unwrap(),
        }));

        let executed_transfer_to_new_op = ExecutedTx {
            signed_tx: transfer_to_new_op.try_get_tx().unwrap().into(),
            success: true,
            op: Some(transfer_to_new_op),
            fail_reason: None,
            block_index,
            created_at: self.get_tx_time(),
            batch_id: None,
        };

        ExecutedOperations::Tx(Box::new(executed_transfer_to_new_op))
    }

    fn create_transfer_tx(&mut self, block_index: Option<u32>) -> ExecutedOperations {
        let transfer_op = ZkSyncOp::Transfer(Box::new(TransferOp {
            tx: self
                .from_zksync_account
                .sign_transfer(
                    self.tokens[1].id,
                    &self.tokens[1].symbol,
                    self.amount.clone(),
                    0u32.into(),
                    &self.to_zksync_account.address,
                    None,
                    true,
                    Default::default(),
                )
                .0,
            from: self.from_zksync_account.get_account_id().unwrap(),
            to: self.to_zksync_account.get_account_id().unwrap(),
        }));

        let executed_transfer_op = ExecutedTx {
            signed_tx: transfer_op.try_get_tx().unwrap().into(),
            success: block_index.is_some(),
            op: Some(transfer_op),
            fail_reason: None,
            block_index,
            created_at: self.get_tx_time(),
            batch_id: None,
        };

        ExecutedOperations::Tx(Box::new(executed_transfer_op))
    }

    fn create_withdraw_tx(&mut self, block_index: Option<u32>) -> ExecutedOperations {
        let withdraw_op = ZkSyncOp::Withdraw(Box::new(WithdrawOp {
            tx: self
                .from_zksync_account
                .sign_withdraw(
                    self.tokens[2].id,
                    &self.tokens[2].symbol,
                    self.amount.clone(),
                    0u32.into(),
                    &self.to_zksync_account.address,
                    None,
                    true,
                    Default::default(),
                )
                .0,
            account_id: self.from_zksync_account.get_account_id().unwrap(),
        }));

        let executed_withdraw_op = ExecutedTx {
            signed_tx: withdraw_op.try_get_tx().unwrap().into(),
            success: true,
            op: Some(withdraw_op),
            fail_reason: None,
            block_index,
            created_at: self.get_tx_time(),
            batch_id: None,
        };

        ExecutedOperations::Tx(Box::new(executed_withdraw_op))
    }

    fn create_close_tx(&mut self, block_index: Option<u32>) -> ExecutedOperations {
        let close_op = ZkSyncOp::Close(Box::new(CloseOp {
            tx: self.from_zksync_account.sign_close(None, false),
            account_id: self.from_zksync_account.get_account_id().unwrap(),
        }));

        let executed_close_op = ExecutedTx {
            signed_tx: close_op.try_get_tx().unwrap().into(),
            success: true,
            op: Some(close_op),
            fail_reason: None,
            block_index,
            created_at: self.get_tx_time(),
            batch_id: None,
        };

        ExecutedOperations::Tx(Box::new(executed_close_op))
    }

    fn create_change_pubkey_tx(&mut self, block_index: Option<u32>) -> ExecutedOperations {
        let change_pubkey_op = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
            tx: self.from_zksync_account.sign_change_pubkey_tx(
                None,
                false,
                TokenId(0),
                Default::default(),
                ChangePubKeyType::ECDSA,
                Default::default(),
            ),
            account_id: self.from_zksync_account.get_account_id().unwrap(),
        }));

        let executed_change_pubkey_op = ExecutedTx {
            signed_tx: change_pubkey_op.try_get_tx().unwrap().into(),
            success: true,
            op: Some(change_pubkey_op),
            fail_reason: None,
            block_index,
            created_at: self.get_tx_time(),
            batch_id: None,
        };

        ExecutedOperations::Tx(Box::new(executed_change_pubkey_op))
    }

    /// This method is important, since it seems that during database roundtrip timestamp
    /// can be rounded and loose several microseconds in precision, which have lead to the
    /// test failures (txs were using `chrono::Utc::now()` and had difference of 1-2 microsecond
    /// between them, which was lost after loading from the DB => multiple txs had the same
    /// timestamp and order could vary from call to call).
    fn get_tx_time(&mut self) -> DateTime<Utc> {
        let current_time = self.next_tx_time;
        self.next_tx_time = current_time + Duration::milliseconds(10);

        current_time
    }
}
