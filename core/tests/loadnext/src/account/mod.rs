use std::{
    future::Future,
    time::{Duration, Instant},
};

use futures::{channel::mpsc::Sender, SinkExt};

use zksync::{error::ClientError, operations::SyncTransactionHandle, RpcProvider, Wallet};
use zksync_eth_signer::PrivateKeySigner;
use zksync_types::{Token, H256};

use crate::{
    account_pool::{AddressPool, TestWallet},
    command::{Command, ExpectedOutcome, IncorrectnessModifier, TxCommand},
    config::LoadtestConfig,
    constants::{COMMIT_TIMEOUT, POLLING_INTERVAL},
    report::{Report, ReportBuilder, ReportLabel},
    rng::LoadtestRng,
};

mod batch_command_executor;
mod tx_command_executor;

/// Account lifespan represents a flow of a single account:
/// it will send transactions and batches, both correct and incorrect, and will check
/// whether outcome matches expected one.
///
/// This structure is expected to not care about the server behavior; even if the server is down, it will only cause
/// performed actions to be considered failed.
#[derive(Debug)]
pub struct AccountLifespan {
    /// Wallet used to perform the test.
    pub wallet: Wallet<PrivateKeySigner, RpcProvider>,
    /// Ethereum private key of the used wallet.
    /// zkSync private key can be obtained from it using `private_key_from_seed` function.
    eth_pk: H256,
    /// Rng unique to the account.
    rng: LoadtestRng,
    config: LoadtestConfig,
    /// Pool of account addresses, used to generate commands.
    addresses: AddressPool,
    /// ERC-20 token used in the test.
    main_token: Token,
    /// Channel for sending reports about performed operations.
    report_sink: Sender<Report>,
}

impl AccountLifespan {
    pub fn new(
        config: &LoadtestConfig,
        addresses: AddressPool,
        test_account: TestWallet,
        report_sink: Sender<Report>,
    ) -> Self {
        let main_token = test_account
            .wallet
            .tokens
            .resolve(config.main_token.as_str().into())
            .unwrap();

        Self {
            wallet: test_account.wallet,
            eth_pk: test_account.eth_pk,
            rng: test_account.rng,
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
            match self.wallet.update_account_id().await {
                Ok(()) => {
                    // Account updated, we're done.
                    break;
                }
                Err(_err) => {
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
        }

        let command_sequence = self.generate_commands();
        for command in command_sequence {
            self.execute_command(command).await;
        }
    }

    /// Executes a command with support of retries:
    /// If command fails due to the network/API error, it will be retried multiple times
    /// before considering it completely failed. Such an approach makes us a bit more resilient to
    /// volatile errors such as random connection drop or insufficient fee error.
    async fn execute_command(&mut self, command: Command) {
        // We consider API errors to be somewhat likely, thus we will retry the operation if it fails
        // due to connection issues.
        const MAX_RETRIES: usize = 3;

        let mut attempt = 0;
        loop {
            let start = Instant::now();
            let result = match &command {
                Command::SingleTx(tx_command) => self.execute_tx_command(tx_command).await,
                Command::Batch(tx_commands) => {
                    self.execute_batch_command(tx_commands.as_ref()).await
                }
                Command::ApiRequest(_) => {
                    todo!()
                }
            };

            let label = match result {
                Ok(label) => label,
                Err(ClientError::NetworkError(_)) | Err(ClientError::OperationTimeout) => {
                    if attempt < MAX_RETRIES {
                        // Retry operation.
                        attempt += 1;
                        continue;
                    }

                    // We reached the maximum amount of retries.
                    let error = format!(
                        "Retries limit reached. Latest error: {}",
                        result.unwrap_err()
                    );
                    ReportLabel::failed(&error)
                }
                Err(err) => {
                    // Other kinds of errors should not be handled, we will just report them.
                    ReportLabel::failed(&err.to_string())
                }
            };

            // We won't continue the loop unless `continue` was manually called.
            self.report(label, start.elapsed(), attempt, command).await;
            break;
        }
    }

    /// Builds a report and sends it.
    async fn report(
        &mut self,
        label: ReportLabel,
        time: Duration,
        retries: usize,
        command: Command,
    ) {
        if matches!(label, ReportLabel::ActionFailed { .. }) {
            vlog::error!("Command failed: {:#?}", command);
        }

        let report = ReportBuilder::new()
            .label(label)
            .reporter(self.wallet.address())
            .time(time)
            .retries(retries)
            .action(command)
            .finish();

        if let Err(_err) = self.report_sink.send(report).await {
            // It's not that important if report will be skipped.
            vlog::trace!("Failed to send report to the sink");
        };
    }

    /// Generic sumbitter for zkSync network: it can operate both individual transactions and
    /// batches, as long as we can provide a `SyncTransactionHandle` to wait for the commitment and the
    /// execution result.
    /// Once result is obtained, it's compared to the expected operation outcome in order to check whether
    /// command was completed as planned.
    async fn submit<F, Fut>(
        &self,
        modifier: IncorrectnessModifier,
        send: F,
    ) -> Result<ReportLabel, ClientError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<SyncTransactionHandle<RpcProvider>, ClientError>>,
    {
        let expected_outcome = modifier.expected_outcome();

        let send_result = send().await;
        let mut handle = match (expected_outcome, send_result) {
            (ExpectedOutcome::ApiRequestFailed, Ok(_handle)) => {
                // Transaction got accepted, but should have not been.
                let error = "Tx/batch was accepted, but should have not been";
                return Ok(ReportLabel::failed(&error));
            }
            (_, Ok(handle)) => {
                // Transaction should have been accepted by API and it was; now wait for the commitment.
                handle
            }
            (ExpectedOutcome::ApiRequestFailed, Err(_error)) => {
                // Transaction was expected to be rejected and it was.
                return Ok(ReportLabel::done());
            }
            (_, Err(_error)) => {
                // Transaction was expected to be accepted, but was rejected.
                let error = "Tx/batch should have been accepted, but got rejected";
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
                    "Unexpected transaction status: expected {:#?} because of modifier {:?}, receipt {:#?}",
                    other, modifier, transaction_receipt
                );
                Ok(ReportLabel::failed(&error))
            }
        }
    }

    /// Prepares a list of random operations to be executed by an account.
    fn generate_commands(&mut self) -> Vec<Command> {
        // We start with a CPK just to unlock accounts.
        let mut commands = vec![Command::SingleTx(TxCommand::change_pubkey(
            self.wallet.address(),
        ))];

        for _ in 0..self.config.operations_per_account {
            let command = Command::random(&mut self.rng, self.wallet.address(), &self.addresses);
            commands.push(command)
        }

        commands
    }
}
