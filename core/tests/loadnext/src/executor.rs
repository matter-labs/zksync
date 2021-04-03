use std::time::Duration;

use futures::{channel::mpsc, future::join_all};

use tokio::task::JoinHandle;
use zksync::{ethereum::PriorityOpHolder, operations::SyncTransactionHandle, provider::Provider};
use zksync_types::{TransactionReceipt, TxFeeTypes, U256};

use crate::constants::*;
use crate::{account::AccountLifespan, account_pool::AccountPool, config::LoadtestConfig};

#[derive(Debug)]
pub struct Executor {
    config: LoadtestConfig,

    pool: AccountPool,
}

impl Executor {
    pub async fn new(config: LoadtestConfig) -> Self {
        let pool = AccountPool::new(&config).await;

        Self { config, pool }
    }

    pub async fn init_accounts(&mut self) -> anyhow::Result<()> {
        vlog::info!("Initializing accounts");
        self.check_onchain_balance().await?;
        self.mint().await?;
        self.deposit_to_master().await?;
        self.set_signing_key().await?;
        let account_futures = self.send_initial_transfers().await?;
        self.wait_account_routines(account_futures).await;

        Ok(())
    }

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

    async fn send_initial_transfers(&mut self) -> anyhow::Result<Vec<JoinHandle<()>>> {
        vlog::info!("Master Account: Sending initial transfers");
        // 40 is a safe limit for now.
        const MAX_TXS_PER_BATCH: usize = 20;
        // How many times we will resend a batch.
        const MAX_RETRIES: usize = 3;

        // Prepare channels for the report collector.
        let (report_sender, _report_receiver) = mpsc::channel(65535);

        let account_balance = self.amount_to_deposit();

        let config = &self.config;
        let master_wallet = &mut self.pool.master_wallet;
        let accounts_amount = config.accounts_amount;
        let token = &config.main_token;
        let addresses = self.pool.addresses.clone();

        let for_fees = u64::max_value() >> 24; // Leave some spare funds on the master account for fees.
        let funds_to_distribute = account_balance - u128::from(for_fees);
        let transfer_amount = funds_to_distribute / accounts_amount as u128;

        let mut retry_counter = 0;
        let mut accounts_processed = 0;

        let mut account_futures = Vec::new();
        while accounts_processed != accounts_amount {
            if retry_counter > MAX_RETRIES {
                anyhow::bail!("Reached max amount of retries when sending a batch");
            }

            // We request nonce each time, so that if one iteration was failed, it will be repeated on the next iteration.
            let mut nonce = master_wallet.account_info().await?.committed.nonce;

            let accounts_left = accounts_amount - accounts_processed;
            let accounts_to_process = std::cmp::min(accounts_left, MAX_TXS_PER_BATCH);

            let mut batch = Vec::new();
            let mut batch_fee_types = Vec::new();
            let mut batch_addresses = Vec::new();

            for account_number in 0..accounts_to_process {
                let target_address = self.pool.accounts[account_number].0.address();
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

            vlog::info!(
                "[{}/{}] Prepared the batch",
                accounts_processed,
                accounts_amount
            );

            // Request fee for the batch.
            let batch_fee = master_wallet
                .provider
                .get_txs_batch_fee(batch_fee_types, batch_addresses, token.as_str())
                .await?;

            vlog::info!(
                "[{}/{}] Got the batch fee info",
                accounts_processed,
                accounts_amount
            );

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

            vlog::info!(
                "[{}/{}] Sent txs batch",
                accounts_processed,
                accounts_amount
            );

            // Now we can wait for a single transaction from the batch to be committed.
            let mut tx_handle =
                SyncTransactionHandle::new(batch_tx_hash, master_wallet.provider.clone());
            tx_handle.polling_interval(Duration::from_secs(3)).unwrap();
            let result = tx_handle
                .commit_timeout(Duration::from_secs(1200))
                .wait_for_commit()
                .await?;

            if result.fail_reason.is_some() {
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
            accounts_processed += accounts_to_process;
            vlog::info!(
                "[{}/{}] Batch succeeded",
                accounts_processed,
                accounts_amount
            );

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

        Ok(account_futures)
    }

    async fn wait_account_routines(&self, account_futures: Vec<JoinHandle<()>>) {
        vlog::info!("Waiting for the account futures to be completed...");
        join_all(account_futures).await;
        vlog::info!("All the spawned tasks are completed");
    }

    fn amount_to_deposit(&self) -> u128 {
        u128::max_value() >> 32
    }

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
