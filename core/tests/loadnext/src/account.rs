use std::todo;

use zksync::{
    error::ClientError, operations::SyncTransactionHandle, provider::Provider, RpcProvider, Wallet,
};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{tx::PackedEthSignature, ZkSyncTx};

use crate::{
    account_pool::AddressPool,
    command::{ExpectedOutcome, IncorrectnessModifier, TxCommand, TxType},
    config::LoadtestConfig,
    constants::{COMMIT_TIMEOUT, POLLING_INTERVAL},
};

#[derive(Debug)]
pub struct AccountLifespan {
    pub wallet: Wallet<PrivateKeySigner, RpcProvider>,
    config: LoadtestConfig,
    addresses: AddressPool,
}

impl AccountLifespan {
    pub fn new(
        config: &LoadtestConfig,
        addresses: AddressPool,
        wallet: Wallet<PrivateKeySigner, RpcProvider>,
    ) -> Self {
        Self {
            wallet,
            config: config.clone(),
            addresses,
        }
    }

    pub async fn run(self) {
        let command_sequence = self.generate_commands();

        for command in command_sequence {
            self.execute_command(command).await;
        }
    }

    async fn execute_command(&self, command: TxCommand) {
        // We consider API errors to be somewhat likely, thus we will retry the operation if it fails
        // due to connection issues.
        const MAX_RETRIES: usize = 3;

        let mut attempt = 0;
        while attempt != MAX_RETRIES {
            let result = match command.command_type {
                TxType::ChangePubKey => self.execute_change_pubkey(command).await,
                _ => {
                    todo!()
                }
            };

            match result {
                Ok(()) => {
                    // TODO: At this point we should report the successful execution.
                    break;
                }
                Err(ClientError::NetworkError(_)) | Err(ClientError::OperationTimeout) => {
                    // Retry operation.
                    attempt += 1;
                }
                Err(_err) => {
                    // Other kinds of errors should not be handled, we will just report them.
                    break;
                }
            }
        }
    }

    async fn execute_change_pubkey(&self, command: TxCommand) -> Result<(), ClientError> {
        let mut builder = self.wallet.start_change_pubkey();

        let tx = match command.modifier {
            IncorrectnessModifier::None => {
                builder = builder.fee_token(self.config.main_token.as_str()).unwrap();

                builder
                    .tx()
                    .await
                    .expect("Transaction can't fail at the creation time")
            }
            _ => {
                todo!()
            }
        };
        let eth_signature = None;

        self.handle_transaction(command, tx, eth_signature).await?;

        Ok(())
    }

    async fn handle_transaction(
        &self,
        command: TxCommand,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<(), ClientError> {
        let expected_outcome = command.modifier.expected_outcome();

        let mut handle = match (
            expected_outcome,
            self.wallet
                .provider
                .send_tx(tx, eth_signature)
                .await
                .map(|hash| SyncTransactionHandle::new(hash, self.wallet.provider.clone())),
        ) {
            (ExpectedOutcome::ApiRequestFailed, Ok(_handle)) => {
                // Transaction got accepted, but should have not been.
                todo!()
            }
            (_, Ok(handle)) => handle,
            (ExpectedOutcome::ApiRequestFailed, Err(_error)) => {
                // Transaction was expected to be rejected and it was.
                todo!()
            }
            (_, Err(_error)) => {
                // Transaction was expected to be accepted, but was rejected.
                todo!()
            }
        };

        handle.polling_interval(POLLING_INTERVAL).unwrap();
        let transaction_receipt = handle
            .commit_timeout(COMMIT_TIMEOUT)
            .wait_for_commit()
            .await?;

        match expected_outcome {
            ExpectedOutcome::TxSucceed if transaction_receipt.fail_reason.is_none() => {
                // Transaction succeed and it should have.
            }
            ExpectedOutcome::TxRejected if transaction_receipt.fail_reason.is_some() => {
                // Transaction failed and it should have.
            }
            _ => {
                // Transaction status didn't match expected one.
                todo!()
            }
        }

        Ok(())
    }

    pub fn generate_commands(&self) -> Vec<TxCommand> {
        let mut commands = vec![TxCommand::change_pubkey(self.wallet.address())];

        for _ in 0..self.config.operations_per_account {
            let command = TxCommand::random(&self.addresses);
            commands.push(command)
        }

        commands
    }
}
