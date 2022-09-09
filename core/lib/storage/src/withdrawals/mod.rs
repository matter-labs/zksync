use crate::withdrawals::records::PendingWithdrawal;
use crate::{BigDecimal, QueryResult, StorageProcessor};
use num::{BigUint, Zero};
use std::str::FromStr;
use std::time::Instant;
use zksync_types::withdrawals::{WithdrawalEvent, WithdrawalPendingEvent};
use zksync_types::H256;
use zksync_utils::biguint_to_big_decimal;

pub mod records;
#[derive(Debug)]
pub struct WithdrawalsSchema<'a, 'c>(pub &'a mut StorageProcessor<'c>);

impl<'a, 'c> WithdrawalsSchema<'a, 'c> {
    pub async fn save_pending_withdrawals(
        &mut self,
        withdrawals: &[WithdrawalPendingEvent],
    ) -> QueryResult<()> {
        let start = Instant::now();
        let mut transaction = self.0.start_transaction().await?;

        for withdrawal in withdrawals {
            let amount =
                biguint_to_big_decimal(BigUint::from_str(&withdrawal.amount.to_string()).unwrap());

            sqlx::query!(
                "INSERT INTO withdrawals (account, amount, token_id, withdrawal_type, pending_tx_hash, pending_tx_log_index, pending_tx_block) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (pending_tx_hash, pending_tx_log_index) DO NOTHING",
                withdrawal.recipient.as_bytes(),
                amount,
                withdrawal.token_id.0 as i32,
                withdrawal.withdrawal_type.to_string(),
                withdrawal.tx_hash.as_bytes(),
                withdrawal.log_index as i64,
                withdrawal.block_number as i64
            )
            .execute(transaction.conn())
            .await?;
        }
        transaction.commit().await?;

        metrics::histogram!("sql.withdrawals.save_pending_withdrawals", start.elapsed());
        Ok(())
    }

    pub async fn finalize_withdrawal(&mut self, withdrawal: &WithdrawalEvent) -> QueryResult<()> {
        let mut transaction = self.0.start_transaction().await?;
        let pending_withdrawals = sqlx::query_as!(
            PendingWithdrawal,
            "SELECT * FROM withdrawals \
             WHERE account= $1 AND token_id = $2 AND pending_tx_block < $3 AND withdrawal_tx_hash is NULL \
             ORDER BY pending_tx_block",
            withdrawal.recipient.as_bytes(),
            withdrawal.token_id.0 as i32,
            withdrawal.block_number as i64
        ).fetch_all(transaction.conn())
        .await?;

        let withdrawal_amount =
            biguint_to_big_decimal(BigUint::from_str(&withdrawal.amount.to_string()).unwrap());
        let mut amount = BigDecimal::zero();
        for pending_withdrawal in pending_withdrawals {
            sqlx::query!(
                "UPDATE withdrawals SET withdrawal_tx_hash = $2, withdrawal_tx_block = $3 WHERE id = $1",
                pending_withdrawal.id,
                withdrawal.tx_hash.as_bytes(),
                withdrawal.block_number as i64,
            ).execute(transaction.conn()).await?;
            amount += pending_withdrawal.amount;
            if amount >= withdrawal_amount {
                break;
            }
        }
        transaction.commit().await?;

        Ok(())
    }

    pub async fn get_finalized_withdrawals(
        &mut self,
        tx_hash: H256,
    ) -> QueryResult<Vec<WithdrawalPendingEvent>> {
        let withdrawals = sqlx::query_as!(
            PendingWithdrawal,
            "SELECT * FROM withdrawals WHERE withdrawal_tx_hash = $1",
            tx_hash.as_bytes()
        )
        .fetch_all(self.0.conn())
        .await?;

        Ok(withdrawals
            .into_iter()
            .map(WithdrawalPendingEvent::from)
            .collect())
    }
}
