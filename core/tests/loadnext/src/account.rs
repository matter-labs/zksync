use std::time::Instant;

use futures::{channel::mpsc::Sender, SinkExt};

use zksync::{
    error::ClientError, operations::SyncTransactionHandle, provider::Provider, RpcProvider, Wallet,
};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{tx::PackedEthSignature, Token, ZkSyncTx, H256};

use crate::{
    account_pool::AddressPool,
    command::{ExpectedOutcome, IncorrectnessModifier, TxCommand, TxType},
    config::LoadtestConfig,
    constants::{COMMIT_TIMEOUT, POLLING_INTERVAL},
    corrupted_tx::Corrupted,
    report::{Report, ReportBuilder, ReportLabel, TxActionType},
};

#[derive(Debug)]
pub struct AccountLifespan {
    pub wallet: Wallet<PrivateKeySigner, RpcProvider>,
    eth_pk: H256,
    config: LoadtestConfig,
    addresses: AddressPool,

    main_token: Token,

    report_sink: Sender<Report>,
}

impl AccountLifespan {
    pub fn new(
        config: &LoadtestConfig,
        addresses: AddressPool,
        (wallet, eth_pk): (Wallet<PrivateKeySigner, RpcProvider>, H256),
        report_sink: Sender<Report>,
    ) -> Self {
        let main_token = wallet
            .tokens
            .resolve(config.main_token.as_str().into())
            .unwrap();

        Self {
            wallet,
            eth_pk,
            config: config.clone(),
            addresses,
            main_token,

            report_sink,
        }
    }

    pub async fn run(mut self) {
        let command_sequence = self.generate_commands();

        for command in command_sequence {
            self.execute_command(command).await;
        }
    }

    async fn send_report(&mut self, report: Report) {
        self.report_sink
            .send(report)
            .await
            .map_err(|_err| {
                // It's not that important if report will be skipped.
                vlog::trace!("Failed to send report to the sink");
            })
            .unwrap_or_default();
    }

    async fn execute_command(&mut self, command: TxCommand) {
        // We consider API errors to be somewhat likely, thus we will retry the operation if it fails
        // due to connection issues.
        const MAX_RETRIES: usize = 3;

        let mut attempt = 0;
        loop {
            let start = Instant::now();
            let result = match command.command_type {
                TxType::ChangePubKey => self.execute_change_pubkey(&command).await,
                TxType::TransferToExisting | TxType::TransferToNew => {
                    self.execute_transfer(&command).await
                }
                TxType::WithdrawToOther | TxType::WithdrawToSelf => {
                    self.execute_withdraw(&command).await
                }
                _ => {
                    todo!()
                }
            };

            match result {
                Ok(()) => {
                    let report = ReportBuilder::new()
                        .label(ReportLabel::ActionDone)
                        .reporter(self.wallet.address())
                        .time(start.elapsed())
                        .retries(attempt)
                        .action(TxActionType::from(command.command_type))
                        .finish();

                    self.send_report(report).await;
                }
                Err(ClientError::NetworkError(_)) | Err(ClientError::OperationTimeout) => {
                    if attempt != MAX_RETRIES {
                        // Retry operation.
                        attempt += 1;
                        continue;
                    } else {
                        // We reached the maximum amount of retries.
                        let error = format!(
                            "Retries limit reached. Latest error: {}",
                            result.unwrap_err()
                        );
                        let report = ReportBuilder::new()
                            .label(ReportLabel::failed(&error))
                            .reporter(self.wallet.address())
                            .time(start.elapsed())
                            .retries(attempt)
                            .action(TxActionType::from(command.command_type))
                            .finish();

                        self.send_report(report).await;
                    }
                }
                Err(err) => {
                    // Other kinds of errors should not be handled, we will just report them.
                    let report = ReportBuilder::new()
                        .label(ReportLabel::failed(&err.to_string()))
                        .reporter(self.wallet.address())
                        .time(start.elapsed())
                        .retries(attempt)
                        .action(TxActionType::from(command.command_type))
                        .finish();

                    self.send_report(report).await;
                }
            }

            // We won't continue the loop unless `continue` was manually called.
            break;
        }
    }

    fn tx_creation_error(err: ClientError) -> ClientError {
        // Translate network errors (so operation will be retried), but don't accept other ones.
        // For example, we will retry operation if fee ticker returned an error,
        // but will panic if transaction cannot be signed.
        match err {
            ClientError::NetworkError(_) | ClientError::RpcError(_) => err,
            _ => panic!("Transaction should be correct"),
        }
    }

    fn apply_modifier(
        &self,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
        modifier: IncorrectnessModifier,
    ) -> (ZkSyncTx, Option<PackedEthSignature>) {
        (tx, eth_signature).apply_modifier(
            modifier,
            self.eth_pk,
            self.main_token.symbol.as_ref(),
            self.main_token.decimals,
        )
    }

    async fn execute_change_pubkey(&self, command: &TxCommand) -> Result<(), ClientError> {
        let tx = self
            .wallet
            .start_change_pubkey()
            .fee_token(self.config.main_token.as_str())
            .unwrap()
            .tx()
            .await
            .map_err(Self::tx_creation_error)?;

        let (tx, eth_signature) = self.apply_modifier(tx, None, command.modifier);

        self.handle_transaction(command, tx, eth_signature).await?;

        Ok(())
    }

    async fn execute_transfer(&self, command: &TxCommand) -> Result<(), ClientError> {
        let (tx, eth_signature) = self
            .wallet
            .start_transfer()
            .to(command.to)
            .amount(command.amount.clone())
            .token(self.config.main_token.as_str())
            .unwrap()
            .tx()
            .await
            .map_err(Self::tx_creation_error)?;

        let (tx, eth_signature) = self.apply_modifier(tx, eth_signature, command.modifier);

        self.handle_transaction(command, tx, eth_signature).await?;

        Ok(())
    }

    async fn execute_withdraw(&self, command: &TxCommand) -> Result<(), ClientError> {
        let (tx, eth_signature) = self
            .wallet
            .start_withdraw()
            .to(command.to)
            .amount(command.amount.clone())
            .token(self.config.main_token.as_str())
            .unwrap()
            .tx()
            .await
            .map_err(Self::tx_creation_error)?;

        let (tx, eth_signature) = self.apply_modifier(tx, eth_signature, command.modifier);

        self.handle_transaction(command, tx, eth_signature).await?;

        Ok(())
    }

    async fn handle_transaction(
        &self,
        command: &TxCommand,
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
            let command = TxCommand::random(self.wallet.address(), &self.addresses);
            commands.push(command)
        }

        commands
    }
}
