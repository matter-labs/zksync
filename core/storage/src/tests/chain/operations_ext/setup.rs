// Built-in imports
// External imports
use bigdecimal::BigDecimal;
// Workspace imports
use crypto_exports::franklin_crypto::bellman::pairing::ff::Field;
use models::node::block::{Block, ExecutedOperations, ExecutedPriorityOp, ExecutedTx};
use models::node::operations::{ChangePubKeyOp, FranklinOp};
use models::node::priority_ops::PriorityOp;
use models::node::{
    Address, CloseOp, Deposit, DepositOp, Fr, FullExit, FullExitOp, Token, TransferOp,
    TransferToNewOp, WithdrawOp,
};
use testkit::zksync_account::ZksyncAccount;
// Local imports

pub struct TransactionsHistoryTestSetup {
    pub from_zksync_account: ZksyncAccount,
    pub to_zksync_account: ZksyncAccount,

    pub amount: BigDecimal,

    pub tokens: Vec<Token>,
    pub blocks: Vec<Block>,
}

impl TransactionsHistoryTestSetup {
    pub fn new() -> Self {
        let tokens = vec![
            Token::new(0, Address::zero(), "ETH"),   // used for deposits
            Token::new(1, Address::random(), "DAI"), // used for transfers
            Token::new(2, Address::random(), "FAU"), // used for withdraws
        ];

        let from_account_id = 0xbabe;
        let from_zksync_account = ZksyncAccount::rand();
        from_zksync_account.set_account_id(Some(from_account_id));

        let to_account_id = 0xdcba;
        let to_zksync_account = ZksyncAccount::rand();
        to_zksync_account.set_account_id(Some(to_account_id));

        let amount = BigDecimal::from(1);

        Self {
            from_zksync_account,
            to_zksync_account,

            amount,

            tokens,
            blocks: Vec::new(),
        }
    }

    pub fn add_block(&mut self, block_id: u32) {
        let executed_deposit_op = self.create_deposit_op(0);
        let executed_transfer_to_new_op = self.create_transfer_to_new_op(Some(1));
        let executed_transfer_op = self.create_transfer_tx(Some(2));
        let executed_close_op = self.create_close_tx(Some(3));
        let executed_change_pubkey_op = self.create_change_pubkey_tx(Some(4));
        let executed_withdraw_op = self.create_withdraw_tx(Some(5));
        let executed_full_exit_op = self.create_full_exit_op(6);

        let operations = vec![
            executed_deposit_op,
            executed_full_exit_op,
            executed_transfer_to_new_op,
            executed_transfer_op,
            executed_withdraw_op,
            executed_close_op,
            executed_change_pubkey_op,
        ];

        let block = Block::new(
            block_id,
            Fr::zero(),
            0,
            operations,
            (0, 0), // Not important
            100,
        );

        self.blocks.push(block);
    }

    fn create_deposit_op(&self, block_index: u32) -> ExecutedOperations {
        let deposit_op = FranklinOp::Deposit(Box::new(DepositOp {
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
                serial_id: 0,
                data: deposit_op.try_get_priority_op().unwrap(),
                deadline_block: 0,
                eth_hash: b"1234567890".to_vec(),
            },
            op: deposit_op,
            block_index,
        };

        ExecutedOperations::PriorityOp(Box::new(executed_op))
    }

    fn create_full_exit_op(&self, block_index: u32) -> ExecutedOperations {
        let full_exit_op = FranklinOp::FullExit(Box::new(FullExitOp {
            priority_op: FullExit {
                account_id: self.from_zksync_account.get_account_id().unwrap(),
                eth_address: self.from_zksync_account.address,
                token: self.tokens[2].id,
            },
            withdraw_amount: Some(self.amount.clone()),
        }));

        let executed_op = ExecutedPriorityOp {
            priority_op: PriorityOp {
                serial_id: 0,
                data: full_exit_op.try_get_priority_op().unwrap(),
                deadline_block: 0,
                eth_hash: b"1234567890".to_vec(),
            },
            op: full_exit_op,
            block_index,
        };

        ExecutedOperations::PriorityOp(Box::new(executed_op))
    }

    fn create_transfer_to_new_op(&self, block_index: Option<u32>) -> ExecutedOperations {
        let transfer_to_new_op = FranklinOp::TransferToNew(Box::new(TransferToNewOp {
            tx: self
                .from_zksync_account
                .sign_transfer(
                    self.tokens[1].id,
                    &self.tokens[1].symbol,
                    self.amount.clone(),
                    BigDecimal::from(0),
                    &self.to_zksync_account.address,
                    None,
                    true,
                )
                .0,
            from: self.from_zksync_account.get_account_id().unwrap(),
            to: self.to_zksync_account.get_account_id().unwrap(),
        }));

        let executed_transfer_to_new_op = ExecutedTx {
            tx: transfer_to_new_op.try_get_tx().unwrap(),
            success: true,
            op: Some(transfer_to_new_op),
            fail_reason: None,
            block_index,
            created_at: chrono::Utc::now(),
        };

        ExecutedOperations::Tx(Box::new(executed_transfer_to_new_op))
    }

    fn create_transfer_tx(&self, block_index: Option<u32>) -> ExecutedOperations {
        let transfer_op = FranklinOp::Transfer(Box::new(TransferOp {
            tx: self
                .from_zksync_account
                .sign_transfer(
                    self.tokens[1].id,
                    &self.tokens[1].symbol,
                    self.amount.clone(),
                    BigDecimal::from(0),
                    &self.to_zksync_account.address,
                    None,
                    true,
                )
                .0,
            from: self.from_zksync_account.get_account_id().unwrap(),
            to: self.to_zksync_account.get_account_id().unwrap(),
        }));

        let executed_transfer_op = ExecutedTx {
            tx: transfer_op.try_get_tx().unwrap(),
            success: true,
            op: Some(transfer_op),
            fail_reason: None,
            block_index,
            created_at: chrono::Utc::now(),
        };

        ExecutedOperations::Tx(Box::new(executed_transfer_op))
    }

    fn create_withdraw_tx(&self, block_index: Option<u32>) -> ExecutedOperations {
        let withdraw_op = FranklinOp::Withdraw(Box::new(WithdrawOp {
            tx: self
                .from_zksync_account
                .sign_withdraw(
                    self.tokens[2].id,
                    &self.tokens[2].symbol,
                    self.amount.clone(),
                    BigDecimal::from(0),
                    &self.to_zksync_account.address,
                    None,
                    true,
                )
                .0,
            account_id: self.from_zksync_account.get_account_id().unwrap(),
        }));

        let executed_withdraw_op = ExecutedTx {
            tx: withdraw_op.try_get_tx().unwrap(),
            success: true,
            op: Some(withdraw_op),
            fail_reason: None,
            block_index,
            created_at: chrono::Utc::now(),
        };

        ExecutedOperations::Tx(Box::new(executed_withdraw_op))
    }

    fn create_close_tx(&self, block_index: Option<u32>) -> ExecutedOperations {
        let close_op = FranklinOp::Close(Box::new(CloseOp {
            tx: self.from_zksync_account.sign_close(None, false),
            account_id: self.from_zksync_account.get_account_id().unwrap(),
        }));

        let executed_close_op = ExecutedTx {
            tx: close_op.try_get_tx().unwrap(),
            success: true,
            op: Some(close_op),
            fail_reason: None,
            block_index,
            created_at: chrono::Utc::now(),
        };

        ExecutedOperations::Tx(Box::new(executed_close_op))
    }

    fn create_change_pubkey_tx(&self, block_index: Option<u32>) -> ExecutedOperations {
        let change_pubkey_op = FranklinOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
            tx: self
                .from_zksync_account
                .create_change_pubkey_tx(None, false, false),
            account_id: self.from_zksync_account.get_account_id().unwrap(),
        }));

        let executed_change_pubkey_op = ExecutedTx {
            tx: change_pubkey_op.try_get_tx().unwrap(),
            success: true,
            op: Some(change_pubkey_op),
            fail_reason: None,
            block_index,
            created_at: chrono::Utc::now(),
        };

        ExecutedOperations::Tx(Box::new(executed_change_pubkey_op))
    }
}
