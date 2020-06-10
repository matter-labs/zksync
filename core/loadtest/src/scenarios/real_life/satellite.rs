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
use models::node::{closest_packable_fee_amount, closest_packable_token_amount};
// Local deps
use crate::{
    rpc_client::RpcClient,
    scenarios::utils::{deposit_single, wait_for_verify},
    sent_transactions::SentTransactions,
    test_accounts::TestAccount,
};

#[derive(Debug)]
pub struct SatelliteScenario {
    rpc_client: RpcClient,
    accounts: Vec<TestAccount>,
    deposit_size: BigUint,
    verify_timeout: Duration,
    estimated_fee_for_op: BigUint,
}

impl SatelliteScenario {
    pub fn new(
        rpc_client: RpcClient,
        accounts: Vec<TestAccount>,
        deposit_size: BigUint,
        verify_timeout: Duration,
    ) -> Self {
        Self {
            rpc_client,
            accounts,
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

        for account_id in 0..self.accounts.len() {
            self.deposit_withdraw(account_id).await?;
        }

        Ok(())
    }

    async fn initialize(&mut self) -> Result<(), failure::Error> {
        for account in self.accounts.iter_mut() {
            account.update_nonce_values(&self.rpc_client).await?;
        }

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

    async fn deposit(&mut self, account_id: usize) -> Result<(), failure::Error> {
        let account = &mut self.accounts[account_id];

        let amount_to_deposit = self.deposit_size.clone() + self.estimated_fee_for_op.clone();

        // Ensure that account does have enough money.
        let account_balance = account.eth_acc.eth_balance().await?;
        if amount_to_deposit > account_balance {
            panic!("Main ETH account does not have enough balance to run the test with the provided config");
        }

        // Deposit funds and wait for operation to be executed.
        deposit_single(account, amount_to_deposit, &self.rpc_client).await?;

        // Now when deposits are done it is time to update account id.
        account.update_account_id(&self.rpc_client).await?;

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (change_pubkey_tx, eth_sign) = (account.sign_change_pubkey(), None);
        let mut sent_txs = SentTransactions::new();
        let tx_hash = self.rpc_client.send_tx(change_pubkey_tx, eth_sign).await?;
        sent_txs.add_tx_hash(tx_hash);
        wait_for_verify(sent_txs, self.verify_timeout, &self.rpc_client).await?;

        Ok(())
    }

    async fn withdraw(&mut self, account_id: usize) -> Result<(), failure::Error> {
        let account = &mut self.accounts[account_id];

        let current_balance = account.eth_acc.eth_balance().await?;

        let fee = self
            .rpc_client
            .get_tx_fee("Withdraw", account.eth_acc.address, "ETH")
            .await
            .expect("Can't get tx fee");

        let fee = closest_packable_fee_amount(&fee);

        let comitted_account_state = self
            .rpc_client
            .account_state_info(account.zk_acc.address)
            .await?
            .committed;
        let account_balance = comitted_account_state.balances["ETH"].0.clone();
        let withdraw_amount = &account_balance - &fee;
        let withdraw_amount = closest_packable_token_amount(&withdraw_amount);

        let (tx, eth_sign) = account.sign_withdraw(withdraw_amount.clone(), fee);
        let tx_hash = self
            .rpc_client
            .send_tx(tx.clone(), eth_sign.clone())
            .await?;
        let mut sent_txs = SentTransactions::new();
        sent_txs.add_tx_hash(tx_hash);

        wait_for_verify(sent_txs, self.verify_timeout, &self.rpc_client).await?;

        let expected_balance = current_balance + withdraw_amount;

        let timeout_minutes = 10;
        let timeout = Duration::from_secs(timeout_minutes * 60);
        let start = Instant::now();

        let polling_interval = Duration::from_millis(250);
        let mut timer = time::interval(polling_interval);

        loop {
            let current_balance = account.eth_acc.eth_balance().await?;
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
}
