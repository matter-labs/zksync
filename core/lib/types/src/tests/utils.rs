use crate::tx::ChangePubKey;
use crate::*;
use chrono::Utc;

pub fn create_full_exit_op() -> ExecutedOperations {
    let priority_op = FullExit {
        account_id: AccountId(0),
        eth_address: Address::zero(),
        token: TokenId(0),
        is_legacy: false,
    };
    ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
        priority_op: PriorityOp {
            serial_id: 0,
            data: ZkSyncPriorityOp::FullExit(priority_op.clone()),
            deadline_block: 0,
            eth_hash: H256::zero(),
            eth_block: 0,
            eth_block_index: None,
        },
        op: ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: None,
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        })),
        block_index: 0,
        created_at: Utc::now(),
    }))
}

pub fn create_withdraw_tx() -> ExecutedOperations {
    let withdraw_op = ZkSyncOp::Withdraw(Box::new(WithdrawOp {
        tx: Withdraw::new(
            AccountId(0),
            Default::default(),
            Default::default(),
            TokenId(0),
            100u32.into(),
            10u32.into(),
            Nonce(12),
            Default::default(),
            None,
        ),
        account_id: AccountId(0),
    }));

    let executed_withdraw_op = ExecutedTx {
        signed_tx: withdraw_op.try_get_tx().unwrap().into(),
        success: true,
        op: Some(withdraw_op),
        fail_reason: None,
        block_index: None,
        created_at: Utc::now(),
        batch_id: None,
    };

    ExecutedOperations::Tx(Box::new(executed_withdraw_op))
}

pub fn create_change_pubkey_tx() -> ExecutedOperations {
    let change_pubkey_op = ZkSyncOp::ChangePubKeyOffchain(Box::new(ChangePubKeyOp {
        tx: ChangePubKey::new(
            AccountId(1),
            Default::default(),
            Default::default(),
            TokenId(0),
            Default::default(),
            Default::default(),
            Default::default(),
            None,
            None,
        ),
        account_id: AccountId(0),
    }));

    let executed_change_pubkey_op = ExecutedTx {
        signed_tx: change_pubkey_op.try_get_tx().unwrap().into(),
        success: true,
        op: Some(change_pubkey_op),
        fail_reason: None,
        block_index: None,
        created_at: Utc::now(),
        batch_id: None,
    };

    ExecutedOperations::Tx(Box::new(executed_change_pubkey_op))
}
