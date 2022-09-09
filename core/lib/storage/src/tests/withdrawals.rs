use zksync_types::withdrawals::{WithdrawalEvent, WithdrawalPendingEvent, WithdrawalType};
use zksync_types::{Address, H256, U256};

use crate::tests::db_test;
use crate::{QueryResult, StorageProcessor};

#[db_test]
async fn test_finalizing(mut storage: StorageProcessor<'_>) -> QueryResult<()> {
    let pending_tx_hash = H256::random();
    let recipient = Address::random();
    storage
        .withdrawals_schema()
        .save_pending_withdrawals(&[
            WithdrawalPendingEvent {
                block_number: 10,
                tx_hash: pending_tx_hash,
                token_id: Default::default(),
                recipient,
                amount: U256::from(10u8),
                withdrawal_type: WithdrawalType::Withdrawal,
                log_index: 0,
            },
            WithdrawalPendingEvent {
                block_number: 10,
                tx_hash: pending_tx_hash,
                token_id: Default::default(),
                recipient,
                amount: U256::from(10u8),
                withdrawal_type: WithdrawalType::ForcedExit,
                log_index: 1,
            },
        ])
        .await?;

    let withdrawal_tx_hash_1 = H256::random();
    storage
        .withdrawals_schema()
        .finalize_withdrawal(&WithdrawalEvent {
            block_number: 11,
            token_id: Default::default(),
            recipient,
            amount: U256::from(10u8),
            tx_hash: withdrawal_tx_hash_1,
        })
        .await?;

    let withdrawals = storage
        .withdrawals_schema()
        .get_finalized_withdrawals(withdrawal_tx_hash_1)
        .await?;

    assert_eq!(withdrawals.len(), 1);

    assert_eq!(withdrawals[0].withdrawal_type, WithdrawalType::Withdrawal);

    let withdrawal_tx_hash_2 = H256::random();
    storage
        .withdrawals_schema()
        .finalize_withdrawal(&WithdrawalEvent {
            block_number: 11,
            token_id: Default::default(),
            recipient,
            amount: U256::from(10u8),
            tx_hash: withdrawal_tx_hash_2,
        })
        .await?;

    let withdrawals = storage
        .withdrawals_schema()
        .get_finalized_withdrawals(withdrawal_tx_hash_2)
        .await?;

    assert_eq!(withdrawals.len(), 1);
    assert_eq!(withdrawals[0].withdrawal_type, WithdrawalType::ForcedExit);
    Ok(())
}
