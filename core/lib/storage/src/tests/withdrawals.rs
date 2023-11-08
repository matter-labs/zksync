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
                amount: U256::from(5u8),
                withdrawal_type: WithdrawalType::ForcedExit,
                log_index: 1,
            },
            WithdrawalPendingEvent {
                block_number: 10,
                tx_hash: pending_tx_hash,
                token_id: Default::default(),
                recipient,
                amount: U256::from(5u8),
                withdrawal_type: WithdrawalType::Withdrawal,
                log_index: 2,
            },
            WithdrawalPendingEvent {
                block_number: 10,
                tx_hash: pending_tx_hash,
                token_id: Default::default(),
                recipient,
                amount: U256::from(5u8),
                withdrawal_type: WithdrawalType::ForcedExit,
                log_index: 3,
            },
        ])
        .await?;

    let withdrawal_tx_hash_1 = H256::random();
    storage
        .withdrawals_schema()
        .finalize_withdrawal(&WithdrawalEvent {
            block_number: 10,
            log_index: 4,
            token_id: Default::default(),
            recipient,
            amount: U256::from(2u8),
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
            block_number: 10,
            log_index: 5,
            token_id: Default::default(),
            recipient,
            amount: U256::from(8u8),
            tx_hash: withdrawal_tx_hash_2,
        })
        .await?;

    let withdrawals = storage
        .withdrawals_schema()
        .get_finalized_withdrawals(withdrawal_tx_hash_2)
        .await?;
    assert_eq!(withdrawals.len(), 1);
    assert_eq!(withdrawals[0].withdrawal_type, WithdrawalType::Withdrawal);

    let withdrawal_tx_hash_3 = H256::random();
    storage
        .withdrawals_schema()
        .finalize_withdrawal(&WithdrawalEvent {
            block_number: 11,
            log_index: 2,
            token_id: Default::default(),
            recipient,
            amount: U256::from(10u8),
            tx_hash: withdrawal_tx_hash_3,
        })
        .await?;

    let withdrawals = storage
        .withdrawals_schema()
        .get_finalized_withdrawals(withdrawal_tx_hash_3)
        .await?;

    assert_eq!(withdrawals.len(), 2);
    assert_eq!(withdrawals[0].withdrawal_type, WithdrawalType::ForcedExit);
    assert_eq!(withdrawals[1].withdrawal_type, WithdrawalType::Withdrawal);
    // Do not process the finalize withdrawal twice
    storage
        .withdrawals_schema()
        .finalize_withdrawal(&WithdrawalEvent {
            block_number: 11,
            log_index: 2,
            token_id: Default::default(),
            recipient,
            amount: U256::from(10u8),
            tx_hash: withdrawal_tx_hash_3,
        })
        .await?;
    let result: Option<i64> = sqlx::query_scalar!(
        "SELECT COUNT(*) \
        FROM withdrawals FULL OUTER JOIN finalized_withdrawals \
        ON finalized_withdrawals.pending_withdrawals_id = withdrawals.id \
        WHERE finalized_withdrawals.tx_hash IS NULL"
    )
    .fetch_one(storage.conn())
    .await
    .unwrap();
    assert_eq!(result.unwrap(), 1);
    Ok(())
}
