use std::{
    convert::TryInto,
    time::{Duration, Instant},
};

use futures::{channel::mpsc::Sender, SinkExt};

use num::BigUint;
use zksync::{
    error::ClientError, ethereum::PriorityOpHolder, operations::SyncTransactionHandle,
    provider::Provider, RpcProvider, Wallet,
};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{tx::PackedEthSignature, Token, TokenId, ZkSyncTx, H256};

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
        // We assume that account is initialized after the transfer to it is executed,
        // thus we can start from obtaining the account ID.
        let retry_attempts = 3;
        for attempt in 0..3 {
            if self.wallet.update_account_id().await.is_err() {
                if attempt == retry_attempts - 1 {
                    // We were not able to obtain the account ID.
                    // Without it, the whole flow cannot be done.
                    vlog::warn!(
                        "Unable to set account ID for account {}",
                        self.wallet.address()
                    );
                    return;
                }
                // We will wait and try again.
                tokio::time::delay_for(Duration::from_secs(1)).await;
            }
        }

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
                TxType::Deposit => self.execute_deposit(&command).await,
                TxType::FullExit => self.execute_full_exit().await,
            };

            match result {
                Ok(label) => {
                    let report = ReportBuilder::new()
                        .label(label)
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
            ClientError::NetworkError(_)
            | ClientError::RpcError(_)
            | ClientError::MalformedResponse(_) => err,
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

    /// Returns the balances for ETH and the main token on the L1.
    /// This function is used to check whether the L1 operation can be performed or should be
    /// skipped.
    async fn l1_balances(&self) -> Result<(BigUint, BigUint), ClientError> {
        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;
        let eth_balance = ethereum.balance().await?;
        let erc20_balance = ethereum
            .erc20_balance(self.wallet.address(), self.main_token.id)
            .await?;

        // Casting via `low_u128` is safe here, since we don't use numbers higher than `u128::max_value()`.
        let erc20_balance = erc20_balance.low_u128().into();
        Ok((eth_balance, erc20_balance))
    }

    async fn execute_deposit(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
        let balances = self.l1_balances().await?;
        if balances.0 == 0u64.into() || balances.1 < command.amount {
            // We don't have either funds in L1 to pay for tx or to deposit.
            // It's not a problem with the server, thus we mark this operation as skipped.
            return Ok(ReportLabel::skipped("No L1 balance"));
        }

        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;

        // We should check whether we've previously approved ERC-20 deposits.
        let deposits_allowed = ethereum
            .is_erc20_deposit_approved(self.main_token.id)
            .await?;
        if !deposits_allowed {
            ethereum
                .approve_erc20_token_deposits(self.main_token.id)
                .await?;
        }

        // Convert BigUint into U256. We won't ever use values above `u128::max_value()`, but just in case we'll ever
        // met such a value, we'll truncate it to the limit.
        let amount = command
            .amount
            .clone()
            .try_into()
            .unwrap_or_else(|_| u128::max_value())
            .into();
        let eth_tx_hash = ethereum
            .deposit(self.main_token.id, amount, self.wallet.address())
            .await?;

        self.handle_priority_op(eth_tx_hash).await
    }

    async fn execute_full_exit(&self) -> Result<ReportLabel, ClientError> {
        let balances = self.l1_balances().await?;
        if balances.0 == 0u64.into() {
            // We don't have either funds in L1 to pay for tx.
            return Ok(ReportLabel::skipped("No L1 balance"));
        }

        // We always call full exit for the ETH, since we don't want to leave the wallet without main token.
        let exit_token_id = TokenId(0);

        let account_id = match self.wallet.account_id() {
            Some(id) => id,
            None => {
                return Ok(ReportLabel::skipped("L2 account was not initialized yet"));
            }
        };

        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;
        let eth_tx_hash = ethereum.full_exit(exit_token_id, account_id).await?;

        self.handle_priority_op(eth_tx_hash).await
    }

    async fn handle_priority_op(&self, eth_tx_hash: H256) -> Result<ReportLabel, ClientError> {
        let ethereum = self.wallet.ethereum(&self.config.web3_url).await?;
        let receipt = ethereum.wait_for_tx(eth_tx_hash).await?;

        let mut priority_op_handle = match receipt.priority_op_handle(self.wallet.provider.clone())
        {
            Some(handle) => handle,
            None => {
                // Probably we did something wrong, no big deal.
                return Ok(ReportLabel::skipped(
                    "Ethereum transaction for deposit failed",
                ));
            }
        };

        priority_op_handle
            .polling_interval(POLLING_INTERVAL)
            .unwrap();
        priority_op_handle
            .commit_timeout(COMMIT_TIMEOUT)
            .wait_for_commit()
            .await?;

        Ok(ReportLabel::done())
    }

    async fn execute_change_pubkey(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
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

        Ok(ReportLabel::done())
    }

    async fn execute_transfer(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
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

        Ok(ReportLabel::done())
    }

    async fn execute_withdraw(&self, command: &TxCommand) -> Result<ReportLabel, ClientError> {
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

        Ok(ReportLabel::done())
    }

    async fn handle_transaction(
        &self,
        command: &TxCommand,
        tx: ZkSyncTx,
        eth_signature: Option<PackedEthSignature>,
    ) -> Result<ReportLabel, ClientError> {
        let expected_outcome = command.modifier.expected_outcome();

        let send_result = self
            .wallet
            .provider
            .send_tx(tx.clone(), eth_signature)
            .await
            .map(|hash| SyncTransactionHandle::new(hash, self.wallet.provider.clone()));
        let mut handle = match (expected_outcome, send_result) {
            (ExpectedOutcome::ApiRequestFailed, Ok(_handle)) => {
                // Transaction got accepted, but should have not been.
                let error = format!("Transaction was accepted, but should have not been: {:#?}. Used modifier: {:?}", tx, command.modifier);
                return Ok(ReportLabel::failed(&error));
            }
            (_, Ok(handle)) => handle,
            (ExpectedOutcome::ApiRequestFailed, Err(_error)) => {
                // Transaction was expected to be rejected and it was.
                return Ok(ReportLabel::done());
            }
            (_, Err(_error)) => {
                // Transaction was expected to be accepted, but was rejected.
                let error = format!("Transaction should have been accepted, but got rejected: {:#?}. Used modifier: {:?}", tx, command.modifier);
                return Ok(ReportLabel::failed(&error));
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
                Ok(ReportLabel::done())
            }
            ExpectedOutcome::TxRejected if transaction_receipt.fail_reason.is_some() => {
                // Transaction failed and it should have.
                Ok(ReportLabel::done())
            }
            other => {
                // Transaction status didn't match expected one.
                let error = format!(
                    "Unexpected transaction status: expected {:#?}, receipt {:#?}. Tx: {:#?}. Used modifier: {:?}",
                    other,
                    transaction_receipt,
                    tx,
                    command.modifier
                );
                Ok(ReportLabel::failed(&error))
            }
        }
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
