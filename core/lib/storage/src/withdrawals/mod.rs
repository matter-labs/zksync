use crate::withdrawals::records::{ExtendedFinalizedWithdrawal, PendingWithdrawal};
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
                "INSERT INTO withdrawals (account, full_amount, remaining_amount, token_id, withdrawal_type, tx_hash, tx_log_index, tx_block) \
                 VALUES ($1, $2, $2, $3, $4, $5, $6, $7)
                 ON CONFLICT (tx_hash, tx_log_index) DO NOTHING",
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
        // Try to find this log in already processed logs
        let log = sqlx::query_scalar!(
            "SELECT tx_log_index FROM finalized_withdrawals \
            WHERE tx_block = $1 AND tx_hash = $2 AND tx_log_index = $3 \
            LIMIT 1",
            withdrawal.block_number as i64,
            withdrawal.tx_hash.as_bytes(),
            withdrawal.log_index as i64,
        )
        .fetch_optional(transaction.conn())
        .await?;

        // If we have already processed txs from this log, just return
        if log.is_some() {
            return Ok(());
        }

        let pending_withdrawals = sqlx::query_as!(
            PendingWithdrawal,
            "SELECT * FROM withdrawals \
             WHERE account= $1 AND token_id = $2 AND tx_block <= $3 AND remaining_amount > 0 \
             ORDER BY tx_block, tx_log_index",
            withdrawal.recipient.as_bytes(),
            withdrawal.token_id.0 as i32,
            withdrawal.block_number as i64
        )
        .fetch_all(transaction.conn())
        .await?;

        let withdrawal_amount =
            biguint_to_big_decimal(BigUint::from_str(&withdrawal.amount.to_string()).unwrap());
        let mut amount = BigDecimal::zero();
        for pending_withdrawal in pending_withdrawals {
            let mut remaining_amount = pending_withdrawal.remaining_amount;
            let remaining_withdrawal_amount = withdrawal_amount.clone() - amount.clone();

            let charged_amount;
            if remaining_withdrawal_amount < remaining_amount {
                remaining_amount -= remaining_withdrawal_amount.clone();
                charged_amount = remaining_withdrawal_amount;
            } else {
                charged_amount = remaining_amount;
                remaining_amount = BigDecimal::zero();
            }

            sqlx::query!(
                "UPDATE withdrawals SET remaining_amount = $2 WHERE id = $1",
                pending_withdrawal.id,
                remaining_amount
            )
            .execute(transaction.conn())
            .await?;
            sqlx::query!(
                "INSERT INTO finalized_withdrawals \
                 (pending_withdrawals_id, amount, tx_hash, tx_block, tx_log_index) \
                 VALUES ($1, $2, $3, $4, $5)",
                pending_withdrawal.id,
                charged_amount,
                withdrawal.tx_hash.as_bytes(),
                withdrawal.block_number as i64,
                withdrawal.log_index as i64,
            )
            .execute(transaction.conn())
            .await?;
            amount += charged_amount;
            if amount == withdrawal_amount {
                break;
            }
            assert!(
                amount < withdrawal_amount,
                "Amount should never be greater than withdrawal amount {:?} {:?}",
                amount,
                withdrawal_amount
            );
        }
        transaction.commit().await?;

        Ok(())
    }

    pub async fn get_finalized_withdrawals(
        &mut self,
        tx_hash: H256,
    ) -> QueryResult<Vec<WithdrawalPendingEvent>> {
        let withdrawals = sqlx::query_as!(
            ExtendedFinalizedWithdrawal,
            "SELECT
                withdrawals.account,
                withdrawals.token_id,
                withdrawals.withdrawal_type,
                finalized_withdrawals.amount,
                withdrawals.tx_hash,
                finalized_withdrawals.tx_block,
                finalized_withdrawals.tx_log_index
            FROM finalized_withdrawals \
            INNER JOIN withdrawals \
            ON finalized_withdrawals.pending_withdrawals_id = withdrawals.id \
            WHERE finalized_withdrawals.tx_hash = $1\
            ORDER BY withdrawals.tx_log_index
            ",
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
