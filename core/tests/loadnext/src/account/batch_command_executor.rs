use zksync::{error::ClientError, operations::SyncTransactionHandle};

use crate::{
    account::AccountLifespan,
    command::{TxCommand, TxType},
    constants::{COMMIT_TIMEOUT, POLLING_INTERVAL},
    report::ReportLabel,
};

impl AccountLifespan {
    pub(super) async fn execute_batch_command(
        &mut self,
        batch_command: &[TxCommand],
    ) -> Result<ReportLabel, ClientError> {
        let mut batch = vec![];

        for command in batch_command {
            let (tx, signature) = match command.command_type {
                TxType::TransferToExisting | TxType::TransferToNew => {
                    self.build_transfer(command).await?
                }
                TxType::WithdrawToOther | TxType::WithdrawToSelf => {
                    self.build_withdraw(command).await?
                }
                _ => unreachable!("Other tx types are not suitable for batches"),
            };

            batch.push((tx, signature));
        }

        // Batch result can be identified by a hash of a single transaction from this batch.
        let main_hash = batch[0].0.hash();

        self.wallet.provider.send_txs_batch(batch, None).await?;

        let mut handle = SyncTransactionHandle::new(main_hash, self.wallet.provider.clone());
        handle.polling_interval(POLLING_INTERVAL).unwrap();
        handle
            .commit_timeout(COMMIT_TIMEOUT)
            .wait_for_commit()
            .await?;

        Ok(ReportLabel::done())
    }
}
