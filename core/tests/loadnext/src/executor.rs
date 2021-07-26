use futures::{channel::mpsc, future::join_all};

use tokio::task::JoinHandle;
use zksync::{
    error::ClientError, ethereum::PriorityOpHolder, operations::SyncTransactionHandle,
    provider::Provider, types::TransactionInfo,
};
use zksync_types::{tx::TxHash, TransactionReceipt, TxFeeTypes, U256};

use crate::{
    account::AccountLifespan, account_pool::AccountPool, config::LoadtestConfig,
    report_collector::LoadtestResult,
};
use crate::{constants::*, report_collector::ReportCollector};

/// Executor is the entity capable of running the loadtest flow.
///
/// It takes care of the following topics:
///
/// - Minting the tokens on L1 for the main account.
/// - Depositing tokens to the main account in L2 and unlocking it.
/// - Spawning the report collector.
/// - Distributing the funds among the test wallets.
/// - Spawning account lifespan futures.
/// - Awaiting for all the account futures to complete.
/// - Getting the final test resolution from the report collector.
#[derive(Debug)]
pub struct Executor {
    config: LoadtestConfig,
    pool: AccountPool,
}

impl Executor {
    /// Creates a new Executor entity.
    pub async fn new(config: LoadtestConfig) -> anyhow::Result<Self> {
        let pool = AccountPool::new(&config).await?;

        Ok(Self { config, pool })
    }

    /// Runs the loadtest until the completion.
    pub async fn start(&mut self) -> LoadtestResult {
        // If the error occurs during the main flow, we will consider it as a test failure.
        self.start_inner().await.unwrap_or_else(|err| {
            vlog::error!("Loadtest was interrupted by the following error: {}", err);
            LoadtestResult::TestFailed
        })
    }

    /// Inner representation of `start` function which returns a `Result`, so it can conveniently use `?`.
    async fn start_inner(&mut self) -> anyhow::Result<LoadtestResult> {
        vlog::info!("Initializing accounts");
        self.check_onchain_balance().await?;
        self.mint().await?;
        self.deposit_to_master().await?;
        self.set_signing_key().await?;
        let (executor_future, account_futures) = self.send_initial_transfers().await?;
        self.wait_account_routines(account_futures).await;

        let final_resultion = executor_future.await.unwrap_or(LoadtestResult::TestFailed);

        Ok(final_resultion)
    }

    /// Verifies that onchain ETH balance for the main account is sufficient to run the loadtest.
    async fn check_onchain_balance(&mut self) -> anyhow::Result<()> {
        vlog::info!("Master Account: Checking onchain balance...");
        let master_wallet = &mut self.pool.master_wallet;
        let ethereum = master_wallet.ethereum(&self.config.web3_url).await?;

        let eth_balance = ethereum.balance().await?;
        if eth_balance < 2u64.pow(17).into() {
            anyhow::bail!(
                "ETH balance is too low to safely perform the loadtest: {}",
                eth_balance
            );
        }

        vlog::info!("Master Account: Onchain balance is OK");
        Ok(())
    }

    /// Mints the ERC-20 token on the main wallet.
    async fn mint(&mut self) -> anyhow::Result<()> {
        vlog::info!("Master Account: Minting ERC20 token...");
        let deposit_amount = self.amount_to_deposit();

        let master_wallet = &self.pool.master_wallet;
        let mut ethereum = master_wallet.ethereum(&self.config.web3_url).await?;
        ethereum.set_confirmation_timeout(ETH_CONFIRMATION_TIMEOUT);

        let token = self.config.main_token.as_str();
        let mint_tx_hash = ethereum
            .mint_erc20(token, U256::from(deposit_amount), master_wallet.address())
            .await?;

        let receipt = ethereum.wait_for_tx(mint_tx_hash).await?;
        self.assert_eth_tx_success(&receipt).await;

        let erc20_balance = ethereum
            .erc20_balance(master_wallet.address(), token)
            .await?;
        assert!(
            erc20_balance >= deposit_amount.into(),
            "Minting didn't result in tokens added to balance"
        );

        vlog::info!("Master Account: Minting is OK (balance: {})", erc20_balance);
        Ok(())
    }

    /// Deposits the ERC-20 token to main wallet in L2.
    async fn deposit_to_master(&mut self) -> anyhow::Result<()> {
        vlog::info!("Master Account: Performing a deposit to master");
        let deposit_amount = self.amount_to_deposit();
        let mut ethereum = self
            .pool
            .master_wallet
            .ethereum(&self.config.web3_url)
            .await?;
        ethereum.set_confirmation_timeout(ETH_CONFIRMATION_TIMEOUT);

        // Approve ERC20 deposits.
        let main_token = self.config.main_token.as_str();
        let approve_tx_hash = ethereum.approve_erc20_token_deposits(main_token).await?;
        let receipt = ethereum.wait_for_tx(approve_tx_hash).await?;
        self.assert_eth_tx_success(&receipt).await;

        vlog::info!("Approved ERC20 deposits");

        // Perform the deposit itself.
        let deposit_tx_hash = ethereum
            .deposit(
                main_token,
                U256::from(deposit_amount),
                self.pool.master_wallet.address(),
            )
            .await?;

        // Wait for the corresponding priority operation to be committed in zkSync.
        let receipt = ethereum.wait_for_tx(deposit_tx_hash).await?;
        self.assert_eth_tx_success(&receipt).await;
        let mut priority_op_handle = receipt
            .priority_op_handle(self.pool.master_wallet.provider.clone())
            .unwrap_or_else(|| {
                panic!(
                    "Can't get the handle for the deposit operation: {:?}",
                    receipt
                );
            });

        priority_op_handle
            .polling_interval(POLLING_INTERVAL)
            .unwrap();
        priority_op_handle
            .commit_timeout(COMMIT_TIMEOUT)
            .wait_for_commit()
            .await?;

        // After deposit is committed, we have to update the account ID in the wallet
        // (in case we didn't have one).
        self.pool.master_wallet.update_account_id().await?;
        assert!(
            self.pool.master_wallet.account_id().is_some(),
            "Account ID for master account was not set",
        );

        vlog::info!("Master Account: Deposit is OK");
        Ok(())
    }

    /// Invokes `ChangePubKey` for the main wallet in L2.
    async fn set_signing_key(&mut self) -> anyhow::Result<()> {
        vlog::info!("Master Account: Setting the signing key");
        let mut handle = self
            .pool
            .master_wallet
            .start_change_pubkey()
            .fee_token(self.config.main_token.as_str())
            .unwrap()
            .send()
            .await?;
        handle.polling_interval(POLLING_INTERVAL).unwrap();
        let result = handle
            .commit_timeout(COMMIT_TIMEOUT)
            .wait_for_commit()
            .await?;

        assert!(
            result.fail_reason.is_none(),
            "Unable to set signing key on the main wallet"
        );

        vlog::info!("Master Account: Signing key is OK");
        Ok(())
    }

    async fn send_initial_transfers_batch(
        &self,
        accounts_to_process: usize,
    ) -> anyhow::Result<TxHash> {
        let eth_to_distribute = self.eth_amount_to_distribute().await?;
        let master_wallet = &self.pool.master_wallet;
        let config = &self.config;
        let token = &config.main_token;

        let transfer_amount = self.transfer_amount();

        let ethereum = master_wallet
            .ethereum(&self.config.web3_url)
            .await
            .expect("Can't get Ethereum client");

        // We request nonce each time, so that if one iteration was failed, it will be repeated on the next iteration.
        let mut nonce = master_wallet.account_info().await?.committed.nonce;

        // 1 tx per account + 1 fee tx.
        let batch_txs_amount = accounts_to_process + 1;
        let mut batch = Vec::with_capacity(batch_txs_amount);
        let mut batch_fee_types = Vec::with_capacity(batch_txs_amount);
        let mut batch_addresses = Vec::with_capacity(batch_txs_amount);

        for account in self.pool.accounts.iter().take(accounts_to_process) {
            let target_address = account.wallet.address();

            // Prior to sending funds in L2, we will send funds in L1 for accounts
            // to be able to perform priority operations.
            // We don't actually care whether transactions will be successful or not; at worst we will not use
            // priority operations in test.
            let _ = ethereum
                .transfer("ETH", eth_to_distribute, target_address)
                .await;

            // And then we will prepare an L2 transaction.
            let (tx, signature) = master_wallet
                .start_transfer()
                .to(target_address)
                .amount(transfer_amount)
                .token(token.as_str())?
                .fee(0u64)
                .nonce(nonce)
                .tx()
                .await?;

            let fee_type = tx.get_fee_info().unwrap().0;
            batch_fee_types.push(fee_type);
            batch_addresses.push(target_address);
            batch.push((tx, signature));

            *nonce += 1;
        }

        // Add mock transfer that contains the fee.
        batch_fee_types.push(TxFeeTypes::Transfer);
        batch_addresses.push(master_wallet.address());

        // Request fee for the batch.
        let batch_fee = master_wallet
            .provider
            .get_txs_batch_fee(batch_fee_types, batch_addresses, token.as_str())
            .await?;

        // Add the fee transaction to the batch.
        let (fee_tx, fee_tx_signature) = master_wallet
            .start_transfer()
            .to(master_wallet.address())
            .amount(0u64)
            .token(token.as_str())?
            .fee(batch_fee)
            .nonce(nonce)
            .tx()
            .await?;

        let batch_tx_hash = fee_tx.hash();

        *nonce += 1;
        batch.push((fee_tx, fee_tx_signature));

        master_wallet.provider.send_txs_batch(batch, None).await?;

        Ok(batch_tx_hash)
    }

    /// Returns the amount sufficient for wallets to perform many operations.
    fn transfer_amount(&self) -> u128 {
        let accounts_amount = self.config.accounts_amount;
        let account_balance = self.amount_to_deposit();
        let for_fees = u64::max_value() >> 24; // Leave some spare funds on the master account for fees.
        let funds_to_distribute = account_balance - u128::from(for_fees);
        funds_to_distribute / accounts_amount as u128
    }

    /// Waits for the transaction execution.
    async fn wait_for_sync_tx(&self, tx_hash: TxHash) -> Result<TransactionInfo, ClientError> {
        let mut tx_handle =
            SyncTransactionHandle::new(tx_hash, self.pool.master_wallet.provider.clone());
        tx_handle.polling_interval(POLLING_INTERVAL).unwrap();

        tx_handle
            .commit_timeout(COMMIT_TIMEOUT)
            .wait_for_commit()
            .await
    }

    /// Initializes the loadtest by doing the following:
    ///
    /// - Spawning the `ReportCollector`.
    /// - Distributing ERC-20 token in L2 among test wallets via `Transfer` operation.
    /// - Distributing ETH in L1 among test wallets in order to make them able to perform priority operations.
    /// - Spawning test account routine futures.
    /// - Collecting all the spawned tasks and returning them to the caller.
    async fn send_initial_transfers(
        &mut self,
    ) -> anyhow::Result<(JoinHandle<LoadtestResult>, Vec<JoinHandle<()>>)> {
        vlog::info!("Master Account: Sending initial transfers");
        // How many times we will resend a batch.
        const MAX_RETRIES: usize = 3;

        // Prepare channels for the report collector.
        let (report_sender, report_receiver) = mpsc::channel(256);

        let report_collector = ReportCollector::new(report_receiver);
        let report_collector_future = tokio::spawn(report_collector.run());

        let config = &self.config;
        let accounts_amount = config.accounts_amount;
        let addresses = self.pool.addresses.clone();

        let mut retry_counter = 0;
        let mut accounts_processed = 0;

        let mut account_futures = Vec::new();
        while accounts_processed != accounts_amount {
            if retry_counter > MAX_RETRIES {
                anyhow::bail!("Reached max amount of retries when sending a batch");
            }

            let accounts_left = accounts_amount - accounts_processed;
            let accounts_to_process = std::cmp::min(accounts_left, MAX_BATCH_SIZE);

            let batch_tx_hash = match self.send_initial_transfers_batch(accounts_to_process).await {
                Ok(hash) => hash,
                Err(err) => {
                    vlog::warn!(
                        "Iteration of the initial funds distribution batch failed: {}",
                        err
                    );
                    retry_counter += 1;
                    continue;
                }
            };

            vlog::info!(
                "[{}/{}] Sent txs batch",
                accounts_processed,
                accounts_amount
            );

            // Now we can wait for a single transaction from the batch to be committed.
            let tx_result = self.wait_for_sync_tx(batch_tx_hash).await?;
            if tx_result.fail_reason.is_some() {
                // Have to try once again.
                retry_counter += 1;
                vlog::info!(
                    "[{}/{}] Batch failed, retrying",
                    accounts_processed,
                    accounts_amount
                );
                continue;
            }

            // All is OK, batch was processed.
            retry_counter = 0;
            vlog::info!(
                "[{}/{}] Batch succeeded",
                accounts_processed,
                accounts_amount
            );
            accounts_processed += accounts_to_process;

            // Spawn each account lifespan.
            let new_account_futures =
                self.pool
                    .accounts
                    .drain(..accounts_to_process)
                    .map(|wallet| {
                        let account = AccountLifespan::new(
                            config,
                            addresses.clone(),
                            wallet,
                            report_sender.clone(),
                        );
                        tokio::spawn(account.run())
                    });

            account_futures.extend(new_account_futures);
        }

        assert!(
            self.pool.accounts.is_empty(),
            "Some accounts were not drained"
        );
        vlog::info!("All the initial transfers are completed");

        Ok((report_collector_future, account_futures))
    }

    /// Calculates amount of ETH to be distributed per account in order to make them
    /// able to perform priority operations.
    async fn eth_amount_to_distribute(&self) -> anyhow::Result<U256> {
        let ethereum = self
            .pool
            .master_wallet
            .ethereum(&self.config.web3_url)
            .await
            .expect("Can't get Ethereum client");

        // Assuming that gas prices on testnets are somewhat stable, we will consider it a constant.
        let average_gas_price = ethereum.client().get_gas_price().await?;

        // Amount of gas required per priority operation at max.
        let gas_per_priority_op = 120_000u64;

        // Amount of priority operations expected to be made by account.
        // We assume that 10% of operations made by account will be priority operations.
        let priority_ops_per_account = self.config.operations_per_account / 10;

        Ok(average_gas_price * gas_per_priority_op * priority_ops_per_account)
    }

    /// Waits for all the test account futures to be completed.
    async fn wait_account_routines(&self, account_futures: Vec<JoinHandle<()>>) {
        vlog::info!("Waiting for the account futures to be completed...");
        join_all(account_futures).await;
        vlog::info!("All the spawned tasks are completed");
    }

    /// Returns the amount of funds to be deposited on the main account in L2.
    /// Amount is chosen to be big enough to not worry about precisely calculating the remaining balances on accounts,
    /// but also to not be close to the supported limits in zkSync.
    fn amount_to_deposit(&self) -> u128 {
        u128::max_value() >> 32
    }

    /// Ensures that Ethereum transaction was successfully executed.
    async fn assert_eth_tx_success(&self, receipt: &TransactionReceipt) {
        if receipt.status != Some(1u64.into()) {
            let master_wallet = &self.pool.master_wallet;
            let ethereum = master_wallet
                .ethereum(&self.config.web3_url)
                .await
                .expect("Can't get Ethereum client");
            let failure_reason = ethereum
                .client()
                .failure_reason(receipt.transaction_hash)
                .await
                .expect("Can't connect to the Ethereum node");
            panic!(
                "Ethereum transaction unexpectedly failed.\nReceipt: {:#?}\nFailure reason: {:#?}",
                receipt, failure_reason
            );
        }
    }
}
