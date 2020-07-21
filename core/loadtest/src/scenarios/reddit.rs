//! Loadtest scenario for the Reddit PoC.
//!
//! This test runs the following operations:
//!
//! - 100,000 point claims (minting & distributing points) (i.e. transfers — AG)
//! - 25,000 subscriptions (i.e. creating subscriptions; this can be done fully offchain — AG)
//! - 75,000 one-off points burning (i.e. subscription redemptions: — AG)
//! - 100,000 transfers

// Scenario logic:
// - Create 25.000 users (via change pubkey op)
// - Execute 4 minting txs per user (total of 100.000)
// - Subscribe every user to the community (25.000 subscriptions)
// - Create 3 burning txs per user (75.000 burning txs)
// - Create 4 transfers per user (100.000 transfers)
// Additional: measure time to run the test.

// Built-in deps
use std::{iter::Iterator, time::Duration};
// External deps
use chrono::Utc;
use futures::future::try_join_all;
use num::BigUint;
use tokio::fs;
use web3::transports::{EventLoopHandle, Http};
// Workspace deps
use models::node::{closest_packable_fee_amount, tx::PackedEthSignature, FranklinTx};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::{
    rpc_client::RpcClient,
    scenarios::{configs::RealLifeConfig, ScenarioContext},
};

const N_ACCOUNTS: usize = 25_000;

#[derive(Debug)]
struct ScenarioExecutor {
    rpc_client: RpcClient,

    /// Intermediate account to rotate funds within.
    accounts: Vec<ZksyncAccount>,

    /// Amount of time to wait for one zkSync block to be verified.
    verify_timeout: Duration,

    /// Event loop handle so transport for Eth account won't be invalidated.
    _event_loop_handle: EventLoopHandle,
}

impl ScenarioExecutor {
    /// Creates a real-life scenario executor.
    pub fn new(ctx: &ScenarioContext, rpc_client: RpcClient) -> Self {
        // Load the config for the test from JSON file.
        let config = RealLifeConfig::load(&ctx.config_path);

        // Generate random accounts to rotate funds within.
        let accounts = (0..N_ACCOUNTS).map(|_| ZksyncAccount::rand()).collect();

        // Create a transport for Ethereum account.
        let (_event_loop_handle, _transport) =
            Http::new(&ctx.options.web3_url).expect("http transport start");

        let verify_timeout = Duration::from_secs(config.block_timeout);

        Self {
            rpc_client,
            accounts,

            verify_timeout,

            _event_loop_handle,
        }
    }

    /// Infallible test runner which performs the emergency exit if any step of the test
    /// fails.
    pub async fn run(&mut self) {
        if let Err(error) = self.run_test().await {
            log::error!("Loadtest erred with the following error: {}", error);
        } else {
            log::info!("Loadtest completed successfully");
        }
    }

    /// Method to be used before the scenario.
    /// It stores all the zkSync account keys into a file named
    /// like "loadtest_accounts_2020_05_05_12_23_55.txt"
    /// so the funds left on accounts will not be lost.
    ///
    /// If saving the file fails, the accounts are printed to the log.
    async fn save_accounts(&self) {
        // Timestamp is used to generate unique file name postfix.
        let timestamp = Utc::now();
        let timestamp_str = timestamp.format("%Y_%m_%d_%H_%M_%S").to_string();

        let output_file_name = format!("reddit_accounts_{}.txt", timestamp_str);

        let mut account_list = String::new();

        // Add all the accounts to the string.
        // Debug representations of account contains both zkSync and Ethereum private keys.
        for account in self.accounts.iter() {
            account_list += &format!("{:?}\n", account);
        }

        // If we're unable to save the file, print its contents to the console at least.
        if let Err(error) = fs::write(&output_file_name, &account_list).await {
            log::error!(
                "Storing the account list erred with the following error: {}",
                error
            );
            log::warn!(
                "Printing the account list to the log instead: \n{}",
                account_list
            )
        } else {
            log::info!(
                "Accounts used in this test are saved to the file '{}'",
                &output_file_name
            );
        }
    }

    /// Runs the test step-by-step. Every test step is encapsulated into its own function.
    pub async fn run_test(&mut self) -> Result<(), failure::Error> {
        self.save_accounts().await;

        self.initialize().await?;

        let account_futures: Vec<_> = (0..N_ACCOUNTS)
            .map(|account_id| self.one_account_run(account_id))
            .collect();

        try_join_all(account_futures).await?;

        // After executing these futures we must send one more (random) tx and wait it to be
        // verified. The verification will mean that all the previously sent txs are verified as well.
        // After that, we may check the balances of every account to check if all the txs were executed
        // successfully.

        self.finish().await?;

        Ok(())
    }

    /// Initializes the test, preparing the main account for the interaction.
    async fn initialize(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }

    async fn one_account_run(&self, account_id: usize) -> Result<(), failure::Error> {
        const N_MINT_OPS: usize = 4;
        const N_SUBSCRIPTIONS: usize = 1;
        const N_BURN_FUNDS_OPS: usize = 3;
        const N_TRANSFER_OPS: usize = 4;

        let account = &self.accounts[account_id];

        self.initialize_account(account).await?;

        for _ in 0..N_MINT_OPS {
            self.mint_tokens(account).await?
        }

        for _ in 0..N_SUBSCRIPTIONS {
            self.subscribe(account).await?
        }

        for _ in 0..N_BURN_FUNDS_OPS {
            self.burn_funds(account).await?
        }

        for _ in 0..N_TRANSFER_OPS {
            self.transfer_funds(account).await?
        }

        Ok(())
    }

    async fn initialize_account(&self, _account: &ZksyncAccount) -> Result<(), failure::Error> {
        // TODO

        // 1. Send the `ChangePubKey` tx to add the account to the tree (this behavior must be implemented beforehand).

        Ok(())
    }

    async fn mint_tokens(&self, _account: &ZksyncAccount) -> Result<(), failure::Error> {
        // TODO

        // 1. Create (not signed) minting tx.
        // 2. Call the Service Provider to sign it.
        // 3. Send the tx.

        Ok(())
    }

    async fn subscribe(&self, _account: &ZksyncAccount) -> Result<(), failure::Error> {
        // TODO

        // 1. Create a subscription account.
        // 2. Notify the Service Provider about it.
        // 3. Manually send a subscription tx.

        Ok(())
    }

    async fn burn_funds(&self, _account: &ZksyncAccount) -> Result<(), failure::Error> {
        // TODO
        Ok(())
    }

    async fn transfer_funds(&self, account: &ZksyncAccount) -> Result<(), failure::Error> {
        let transfer_size: u64 = 1;
        let fee = self.transfer_fee(account).await;
        let (tx, eth_sign) = self.sign_transfer(account, account, transfer_size, fee);

        let _tx_hash = self
            .rpc_client
            .send_tx(tx.clone(), eth_sign.clone())
            .await?;

        // We do not wait for the verification.

        Ok(())
    }

    async fn finish(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }

    /// Obtains a fee required for the transfer operation.
    async fn transfer_fee(&self, to_acc: &ZksyncAccount) -> BigUint {
        let fee = self
            .rpc_client
            .get_tx_fee("Transfer", to_acc.address, "ETH")
            .await
            .expect("Can't get tx fee");

        closest_packable_fee_amount(&fee)
    }

    /// Creates a signed transfer transaction.
    /// Sender and receiver are chosen from the generated
    /// accounts, determined by its indices.
    fn sign_transfer(
        &self,
        from: &ZksyncAccount,
        to: &ZksyncAccount,
        amount: impl Into<BigUint>,
        fee: impl Into<BigUint>,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let (tx, eth_signature) = from.sign_transfer(
            0, // ETH
            "ETH",
            amount.into(),
            fee.into(),
            &to.address,
            None,
            true,
        );

        (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
    }
}

/// Runs the real-life test scenario.
/// For description, see the module doc-comment.
pub fn run_scenario(mut ctx: ScenarioContext) {
    let rpc_addr = ctx.rpc_addr.clone();
    let rpc_client = RpcClient::new(&rpc_addr);

    let mut scenario = ScenarioExecutor::new(&ctx, rpc_client);

    // Run the scenario.
    log::info!("Starting the real-life test");
    ctx.rt.block_on(scenario.run());
}
