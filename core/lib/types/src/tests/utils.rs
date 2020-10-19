use crate::tx::ChangePubKey;
use crate::*;
use chrono::Utc;

pub fn create_full_exit_op() -> ExecutedOperations {
    let priority_op = FullExit {
        account_id: 0,
        eth_address: Address::zero(),
        token: 0,
    };
    ExecutedOperations::PriorityOp(Box::new(ExecutedPriorityOp {
        priority_op: PriorityOp {
            serial_id: 0,
            data: ZkSyncPriorityOp::FullExit(priority_op.clone()),
            deadline_block: 0,
            eth_hash: Vec::new(),
            eth_block: 0,
        },
        op: ZkSyncOp::FullExit(Box::new(FullExitOp {
            priority_op,
            withdraw_amount: None,
        })),
        block_index: 0,
        created_at: Utc::now(),
    }))
}

pub fn create_withdraw_tx() -> ExecutedOperations {
    let withdraw_op = ZkSyncOp::Withdraw(Box::new(WithdrawOp {
        tx: Withdraw::new(
            0,
            Default::default(),
            Default::default(),
            0,
            100u32.into(),
            10u32.into(),
            12,
            None,
        ),
        account_id: 0,
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
            1,
            Default::default(),
            Default::default(),
            0,
            Default::default(),
            Default::default(),
            None,
            None,
        ),
        account_id: 0,
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
