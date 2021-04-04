use zksync::{error::ClientError, operations::SyncTransactionHandle};

use crate::{
    account::AccountLifespan,
    command::{IncorrectnessModifier, TxCommand, TxType},
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

        // If we have multiple bad transactions in the batch, the fail reason will be equal to the
        // fail reason of the first incorrect transaction.
        // This goes both to failures on API side and on the state side.
        let modifier = batch_command
            .iter()
            .find_map(|cmd| match cmd.modifier {
                IncorrectnessModifier::None => None,
                other => Some(other),
            })
            .unwrap_or(IncorrectnessModifier::None);

        let provider = self.wallet.provider.clone();
        self.sumbit(modifier, || async {
            provider.send_txs_batch(batch, None).await?;
            Ok(SyncTransactionHandle::new(main_hash, provider))
        })
        .await
    }
}
