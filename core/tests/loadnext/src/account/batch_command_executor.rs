use zksync::{error::ClientError, operations::SyncTransactionHandle};

use crate::{
    account::AccountLifespan,
    command::{ExpectedOutcome, IncorrectnessModifier, TxCommand, TxType},
    report::ReportLabel,
};

impl AccountLifespan {
    pub(super) async fn execute_batch_command(
        &mut self,
        batch_command: &[TxCommand],
    ) -> Result<ReportLabel, ClientError> {
        let mut batch = Vec::with_capacity(batch_command.len());

        // Since we're manually building the batch, we have to increment nonce by ourselves.
        // Otherwise all the pre-built transactions will have the same (currently committed) nonce.
        let mut nonce = self.wallet.account_info().await?.committed.nonce;

        for command in batch_command {
            let (tx, signature) = match command.command_type {
                TxType::TransferToExisting | TxType::TransferToNew => {
                    self.build_transfer(command, Some(nonce)).await?
                }
                TxType::WithdrawToOther | TxType::WithdrawToSelf => {
                    self.build_withdraw(command, Some(nonce)).await?
                }
                TxType::ChangePubKey => self.build_change_pubkey(command, Some(nonce)).await?,
                _ => unreachable!("Other tx types are not suitable for batches"),
            };

            batch.push((tx, signature));
            *nonce += 1;
        }

        // Batch result can be identified by a hash of a single transaction from this batch.
        let main_hash = batch[0].0.hash();

        // If we have multiple bad transactions in the batch, the fail reason will be equal to the
        // fail reason of the first incorrect transaction.
        // This goes both to failures on API side and on the state side.
        // However, if there is a failure reason that will be declined by API, it should be prioritized,
        // since in that case even if the first error must result in a tx rejection by state, it just
        // wouldn't reach there.
        let modifier = batch_command
            .iter()
            .map(|cmd| cmd.modifier)
            .find(|modifier| {
                // First attempt: find the API error-causing topic.
                modifier.expected_outcome() == ExpectedOutcome::ApiRequestFailed
            })
            .unwrap_or_else(|| {
                // Second attempt: find any error-causing topic.
                batch_command
                    .iter()
                    .map(|cmd| cmd.modifier)
                    .find(|modifier| *modifier != IncorrectnessModifier::None)
                    .unwrap_or(IncorrectnessModifier::None)
            });

        let provider = self.wallet.provider.clone();
        self.submit(modifier, || async {
            self.wallet.provider.send_txs_batch(batch, None).await?;
            Ok(SyncTransactionHandle::new(main_hash, provider))
        })
        .await
    }
}
