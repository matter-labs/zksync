//! Satellite scenario for real-life loadtest.
//!
//! Satellite scenario is ran concurrently to the main scenario
//! and it performs several deposit / withdraw operations at the same
//! time as the funds are rotated in the main scenario.
//!
//! The purpose of the satellite scenario is to ensure that deposits
//! and withdraws are processed correctly when the node is under a
//! load of many transfers.

// Built-in deps
use std::time::{Duration, Instant};
// External deps
use num::BigUint;
use tokio::time;
// Workspace deps
use models::{
    helpers::{closest_packable_fee_amount, closest_packable_token_amount},
    TxFeeTypes,
};
use zksync::Provider;
// Local deps
use crate::{
    scenarios::utils::{deposit_single, wait_for_verify},
    sent_transactions::SentTransactions,
    test_accounts::TestWallet,
};

#[derive(Debug)]
pub struct SatelliteScenario {
    provider: Provider,
    wallets: Vec<TestWallet>,
    deposit_size: BigUint,
    verify_timeout: Duration,
    estimated_fee_for_op: BigUint,
}

impl SatelliteScenario {
    pub fn new(
        provider: Provider,
        wallets: Vec<TestWallet>,
        deposit_size: BigUint,
        verify_timeout: Duration,
    ) -> Self {
        Self {
            provider,
            wallets,
            deposit_size,
            verify_timeout,
            estimated_fee_for_op: 0u32.into(),
        }
    }

    pub fn set_estimated_fee(&mut self, estimated_fee_for_op: BigUint) {
        self.estimated_fee_for_op = estimated_fee_for_op
    }

    pub async fn run(&mut self) -> Result<(), failure::Error> {
        self.initialize().await?;

        // Deposit & withdraw phase.
        for account_id in 0..self.wallets.len() {
            self.deposit_withdraw(account_id).await?;
        }

        // Deposit & full exit phase.
        for account_id in 0..self.wallets.len() {
            self.deposit_full_exit(account_id).await?;
        }

        Ok(())
    }

    async fn initialize(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }

    async fn deposit_withdraw(&mut self, account_id: usize) -> Result<(), failure::Error> {
        log::info!(
            "Satellite deposit/withdraw iteration {} started",
            account_id
        );

        self.deposit(account_id).await?;
        log::info!("Satellite deposit iteration {} completed", account_id);

        self.withdraw(account_id).await?;
        log::info!("Satellite withdraw iteration {} completed", account_id);

        Ok(())
    }

    async fn deposit_full_exit(&mut self, account_id: usize) -> Result<(), failure::Error> {
        log::info!(
            "Satellite deposit/full exit iteration {} started",
            account_id
        );

        self.deposit(account_id).await?;
        log::info!("Satellite deposit iteration {} completed", account_id);

        self.full_exit(account_id).await?;
        log::info!("Satellite full exit iteration {} completed", account_id);

        Ok(())
    }

    async fn deposit(&mut self, account_id: usize) -> Result<(), failure::Error> {
        let wallet = &mut self.wallets[account_id];

        let amount_to_deposit = self.deposit_size.clone() + self.estimated_fee_for_op.clone();

        // Ensure that account does have enough money.
        let account_balance = wallet.eth_provider.balance().await?;
        if amount_to_deposit > account_balance {
            panic!("Main ETH account does not have enough balance to run the test with the provided config");
        }

        // Deposit funds and wait for operation to be executed.
        deposit_single(wallet, amount_to_deposit, &self.provider).await?;

        // Now when deposits are done it is time to update account id.
        wallet.update_account_id().await?;

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (change_pubkey_tx, eth_sign) = (wallet.sign_change_pubkey().await?, None);
        let mut sent_txs = SentTransactions::new();
        let tx_hash = self.provider.send_tx(change_pubkey_tx, eth_sign).await?;
        sent_txs.add_tx_hash(tx_hash);
        wait_for_verify(sent_txs, self.verify_timeout, &self.provider).await?;

        Ok(())
    }

    async fn withdraw(&mut self, account_id: usize) -> Result<(), failure::Error> {
        let wallet = &mut self.wallets[account_id];

        let current_balance = wallet.eth_provider.balance().await?;

        let fee = self
            .provider
            .get_tx_fee(
                TxFeeTypes::Withdraw,
                wallet.address(),
                TestWallet::TOKEN_NAME,
            )
            .await
            .expect("Can't get tx fee")
            .total_fee;

        let fee = closest_packable_fee_amount(&fee);

        let comitted_account_state = self
            .provider
            .account_info(wallet.address())
            .await?
            .committed;
        let account_balance = comitted_account_state.balances[TestWallet::TOKEN_NAME]
            .0
            .clone();
        let withdraw_amount = &account_balance - &fee;
        let withdraw_amount = closest_packable_token_amount(&withdraw_amount);

        let (tx, eth_sign) = wallet
            .sign_withdraw(withdraw_amount.clone(), Some(fee))
            .await?;
        let tx_hash = self.provider.send_tx(tx.clone(), eth_sign.clone()).await?;
        let mut sent_txs = SentTransactions::new();
        sent_txs.add_tx_hash(tx_hash);

        wait_for_verify(sent_txs, self.verify_timeout, &self.provider).await?;

        let expected_balance = current_balance + withdraw_amount;

        let timeout_minutes = 10;
        let timeout = Duration::from_secs(timeout_minutes * 60);
        let start = Instant::now();

        let polling_interval = Duration::from_millis(250);
        let mut timer = time::interval(polling_interval);

        loop {
            let current_balance = wallet.eth_provider.balance().await?;
            if current_balance == expected_balance {
                break;
            }
            if start.elapsed() > timeout {
                failure::bail!(
                    "ETH funds were not received for {} minutes",
                    timeout_minutes
                );
            }
            timer.tick().await;
        }

        Ok(())
    }

    async fn full_exit(&mut self, account_id: usize) -> Result<(), failure::Error> {
        let wallet = &mut self.wallets[account_id];

        let zksync_account_id = wallet.account_id().expect("No account ID set");

        wallet
            .eth_provider
            .full_exit(TestWallet::TOKEN_NAME, zksync_account_id)
            .await?;

        Ok(())
    }
}
