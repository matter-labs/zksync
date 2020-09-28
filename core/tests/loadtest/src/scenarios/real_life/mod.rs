//! Real-life loadtest scenario does not measure the TPS nor simulated the high load,
//! but rather simulates the real-life use case of zkSync:
//!
//! 1. Funds are deposited from one Ethereum account into one new zkSync account.
//! 2. Once funds are deposited, this account split the funds between N accounts
//!    using the `transferToNew` operation.
//! 3. Once funds are transferred and verified, these funds are "rotated" within
//!    created accounts using the `transfer` operation. This operation is repeated
//!    M times.
//! 4. To finish the test, all the funds are collected back to the initial account
//!    are withdrawn to the Ethereum.
//!
//! `N` and `M` are configurable parameters, meaning the breadth of the test (how
//! many accounts will be used within the test) and the depth of the test (how
//! many rotation cycles are performed) correspondingly.
//!
//! Schematically, scenario will look like this:
//!
//! ```text
//! Deposit  | Transfer to new  | Transfer | Collect back | Withdraw to ETH
//!          |                  |          |              |
//!          |                  |  ┗━━━━┓  |              |
//!          |           ┏━━━>Acc1━━━━━┓┗>Acc1━━━┓        |
//!          |         ┏━┻━━━>Acc2━━━━┓┗━>Acc2━━━┻┓       |
//! ETH━━━━>InitialAcc━╋━━━━━>Acc3━━━┓┗━━>Acc3━━━━╋━>InitialAcc━>ETH
//!          |         ┗━┳━━━>Acc4━━┓┗━━━>Acc4━━━┳┛       |
//!          |           ┗━━━>Acc5━┓┗━━━━>Acc5━━━┛        |
//! ```
//!
//! ## Test configuration
//!
//! To configure the test, one should provide a JSON config with the following structure:
//!
//! ```json
//! {
//!     "n_accounts": 100,     // Amount of intermediate account to use, "breadth" of the test.
//!     "transfer_size": 100,  // Amount of money to be used in the transfer, in wei.
//!     "cycles_amount": 10,   // Amount of iterations to rotate funds, "length" of the test.
//!     "block_timeout": 120,  // Amount of time to wait for one zkSync block to be verified.
//!     "use_all_block_sizes": false, // Whether to use different block sizes (may slowdown the test execution).
//!     "input_account": {     // Address/private key of the Ethereum account to deposit money for test from.
//!         "address": "0x36615Cf349d7F6344891B1e7CA7C72883F5dc049",
//!         "private_key": "0x7726827caac94a7f9e1b160f7ea819f172f7b6f9d2a97f992c38edeab82d4110"
//!     }
//! }
//! ```
//!
//! `configs` folder of the crate contains a `reallife.json` sample configuration optimized for
//! running the test in development infrastructure (dummy prover, 1 second for one Ethereum block
//! testnet Ethereum chain).

// Built-in deps
use std::{
    iter::Iterator,
    time::{Duration, Instant},
};
// External deps
use chrono::Utc;
use futures::future::try_join_all;
use num::BigUint;
use tokio::{fs, time};
// Workspace deps
use models::{
    helpers::{closest_packable_fee_amount, closest_packable_token_amount},
    tx::PackedEthSignature,
    FranklinTx, TxFeeTypes,
};
use zksync::{Network, Provider};
use zksync_config::ConfigurationOptions;
use zksync_utils::format_ether;
// Local deps
use self::satellite::SatelliteScenario;
use crate::{
    scenarios::{
        configs::RealLifeConfig,
        utils::{deposit_single, wait_for_verify, DynamicChunks},
        ScenarioContext,
    },
    sent_transactions::SentTransactions,
    test_accounts::TestWallet,
};

mod satellite;

#[derive(Debug)]
struct ScenarioExecutor {
    provider: Provider,

    /// Main account to deposit ETH from / return ETH back to.
    main_wallet: TestWallet,

    /// Intermediate account to rotate funds within.
    accounts: Vec<TestWallet>,

    /// Amount of intermediate accounts.
    n_accounts: usize,
    /// Transfer amount per accounts (in wei).
    transfer_size: BigUint,
    /// Amount of cycles for funds rotation.
    cycles_amount: u32,

    /// Block sizes supported by server and suitable to use in this test
    /// (to not overload the node with too many txs at the moment)
    block_sizes: Vec<usize>,

    /// Amount of time to wait for one zkSync block to be verified.
    verify_timeout: Duration,

    /// Estimated fee amount for any zkSync operation. It is used to deposit
    /// funds initially and transfer the funds for intermediate accounts to
    /// operate.
    estimated_fee_for_op: BigUint,

    /// Satellite scenario to run alongside with the funds rotation cycles.
    satellite_scenario: Option<SatelliteScenario>,
}

impl ScenarioExecutor {
    /// Creates a real-life scenario executor.
    pub fn new(ctx: &mut ScenarioContext, provider: Provider) -> Self {
        // Load the config for the test from JSON file.
        let config = RealLifeConfig::load(&ctx.config_path);

        // Generate random accounts to rotate funds within.
        let accounts = (0..config.n_accounts)
            .map(|_| {
                ctx.rt
                    .block_on(TestWallet::new_random(provider.clone(), &ctx.options))
            })
            .collect();

        // Create main account to deposit money from and to return money back later.
        let main_wallet = ctx.rt.block_on(TestWallet::from_info(
            &config.input_account,
            provider.clone(),
            &ctx.options,
        ));

        // Load additional accounts for the satellite scenario.
        let additional_accounts: Vec<_> = config
            .additional_accounts
            .iter()
            .map(|acc| {
                ctx.rt
                    .block_on(TestWallet::from_info(acc, provider.clone(), &ctx.options))
            })
            .collect();

        let block_sizes = Self::get_block_sizes(config.use_all_block_sizes);

        if config.use_all_block_sizes {
            log::info!(
                "Following block sizes will be used in test: {:?}",
                block_sizes
            );
        }

        let transfer_size = closest_packable_token_amount(&BigUint::from(config.transfer_size));
        let verify_timeout = Duration::from_secs(config.block_timeout);

        let satellite_scenario = Some(SatelliteScenario::new(
            provider.clone(),
            additional_accounts,
            transfer_size.clone(),
            verify_timeout,
        ));

        Self {
            provider,

            main_wallet,
            accounts,

            n_accounts: config.n_accounts,
            transfer_size,
            cycles_amount: config.cycles_amount,

            block_sizes,

            verify_timeout,

            estimated_fee_for_op: 0u32.into(),

            satellite_scenario,
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

        let output_file_name = format!("loadtest_accounts_{}.txt", timestamp_str);

        let mut account_list = String::new();

        // Add all the accounts to the string.
        // Debug representations of account contains both zkSync and Ethereum private keys.
        account_list += &format!("{:?}\n", self.main_wallet);
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
        self.deposit().await?;
        self.initial_transfer().await?;

        // Take the satellite scenario, as we have to borrow it mutably.
        let mut satellite_scenario = self.satellite_scenario.take().unwrap();

        // Run funds rotation phase and the satellite scenario in parallel.
        let funds_rotation_future = self.funds_rotation();
        let satellite_scenario_future = satellite_scenario.run();
        futures::try_join!(funds_rotation_future, satellite_scenario_future)?;

        self.collect_funds().await?;
        self.withdraw().await?;
        self.finish().await?;

        Ok(())
    }

    /// Initializes the test, preparing the main account for the interaction.
    async fn initialize(&mut self) -> Result<(), failure::Error> {
        // Then, we have to get the fee value (assuming that dev-ticker is used, we estimate
        // the fee in such a way that it will always be sufficient).
        // Withdraw operation has more chunks, so we estimate fee for it.
        let mut fee = self.withdraw_fee(&self.main_wallet).await;

        // To be sure that we will have enough funds for all the transfers,
        // we will request 1.2x of the suggested fees. All the unspent funds
        // will be withdrawn later.
        fee = fee * BigUint::from(120u32) / BigUint::from(100u32);

        // And after that we have to make the fee packable.
        fee = closest_packable_fee_amount(&fee);

        self.estimated_fee_for_op = fee.clone();

        if let Some(scenario) = self.satellite_scenario.as_mut() {
            scenario.set_estimated_fee(fee);
        };

        Ok(())
    }

    /// Runs the initial deposit of the money onto the main account.
    async fn deposit(&mut self) -> Result<(), failure::Error> {
        // Amount of money we need to deposit.
        // Initialize it with the raw amount: only sum of transfers per account.
        // Fees are taken into account below.
        let mut amount_to_deposit =
            self.transfer_size.clone() * BigUint::from(self.n_accounts as u64);

        // Count the fees: we need to provide fee for each of initial transfer transactions,
        // for each funds rotating transaction, and for each withdraw transaction.

        // Sum of fees for one tx per every account.
        let fee_for_all_accounts =
            self.estimated_fee_for_op.clone() * BigUint::from(self.n_accounts as u64);
        // Total amount of cycles is amount of funds rotation cycles + one for initial transfers +
        // one for collecting funds back to the main account.
        amount_to_deposit += fee_for_all_accounts * (self.cycles_amount + 2);
        // Also the fee is required to perform a final withdraw
        amount_to_deposit += self.estimated_fee_for_op.clone();

        let account_balance = self.main_wallet.eth_provider.balance().await?;
        log::info!(
            "Main account ETH balance: {} ETH",
            format_ether(&account_balance)
        );

        log::info!(
            "Starting depositing phase. Depositing {} ETH to the main account",
            format_ether(&amount_to_deposit)
        );

        // Ensure that account does have enough money.
        if amount_to_deposit > account_balance {
            panic!("Main ETH account does not have enough balance to run the test with the provided config");
        }

        // Deposit funds and wait for operation to be executed.
        deposit_single(&self.main_wallet, amount_to_deposit, &self.provider).await?;

        log::info!("Deposit sent and verified");

        // Now when deposits are done it is time to update account id.
        self.main_wallet.update_account_id().await?;

        log::info!("Main account ID set");

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (change_pubkey_tx, eth_sign) = (self.main_wallet.sign_change_pubkey().await?, None);
        let mut sent_txs = SentTransactions::new();
        let tx_hash = self.provider.send_tx(change_pubkey_tx, eth_sign).await?;
        sent_txs.add_tx_hash(tx_hash);
        wait_for_verify(sent_txs, self.verify_timeout, &self.provider).await?;

        log::info!("Main account pubkey changed");

        log::info!("Deposit phase completed");

        Ok(())
    }

    /// Splits the money from the main account between the intermediate accounts
    /// with the `TransferToNew` operations.
    async fn initial_transfer(&mut self) -> Result<(), failure::Error> {
        log::info!(
            "Starting initial transfer. {} ETH will be send to each of {} new accounts",
            format_ether(&self.transfer_size),
            self.n_accounts
        );

        let mut signed_transfers = Vec::with_capacity(self.n_accounts);

        for to_idx in 0..self.n_accounts {
            let from_acc = &self.main_wallet;
            let to_acc = &self.accounts[to_idx];

            // Transfer size is (transfer_amount) + (fee for every tx to be sent) + (fee for final transfer
            // back to the main account).
            let transfer_amount = self.transfer_size.clone()
                + self.estimated_fee_for_op.clone() * (self.cycles_amount + 1);

            // Make amount packable.
            let packable_transfer_amount = closest_packable_fee_amount(&transfer_amount);

            // Fee for the transfer itself differs from the estimated fee.
            let fee = self.transfer_fee(&to_acc).await;
            let transfer = self
                .sign_transfer(from_acc, to_acc, packable_transfer_amount, fee)
                .await;

            signed_transfers.push(transfer);
        }

        log::info!("Signed all the initial transfer transactions, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        let txs_chunks = DynamicChunks::new(signed_transfers, &self.block_sizes);
        for tx_batch in txs_chunks {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            // This has to be done synchronously, since we're sending from the same account
            // and truly async sending will result in a nonce mismatch errors.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self.provider.send_tx(tx.clone(), eth_sign.clone()).await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            let sent_txs_amount = sent_txs.len();
            verified += sent_txs_amount;

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, self.verify_timeout, &self.provider).await?;

            log::info!(
                "Sent and verified {}/{} txs ({} on this iteration)",
                verified,
                to_verify,
                sent_txs_amount
            );
        }

        log::info!("All the initial transfers are completed");
        log::info!("Updating the accounts info and changing their public keys");

        // After all the initial transfer completed, we have to update new account IDs
        // and change public keys of accounts (so we'll be able to send transfers from them).
        let mut tx_futures = vec![];
        for wallet in self.accounts.iter_mut() {
            let resp = self
                .provider
                .account_info(wallet.address())
                .await
                .expect("rpc error");
            assert!(resp.id.is_some(), "Account ID is none for new account");
            wallet.update_account_id().await?;

            let change_pubkey_tx = wallet.sign_change_pubkey().await?;
            let tx_future = self.provider.send_tx(change_pubkey_tx, None);

            tx_futures.push(tx_future);
        }
        let mut sent_txs = SentTransactions::new();
        sent_txs.tx_hashes = try_join_all(tx_futures).await?;

        // Calculate the estimated amount of blocks for all the txs to be processed.
        let max_block_size = *self.block_sizes.iter().max().unwrap();
        let n_blocks = (self.accounts.len() / max_block_size + 1) as u32;
        wait_for_verify(sent_txs, self.verify_timeout * n_blocks, &self.provider).await?;

        log::info!("All the accounts are prepared");

        log::info!("Initial transfers are sent and verified");

        Ok(())
    }

    /// Performs the funds rotation phase: transfers the money between intermediate
    /// accounts multiple times.
    /// Sine the money amount is always the same, after execution of this step every
    /// intermediate account should have the same balance as it has before.
    async fn funds_rotation(&mut self) -> Result<(), failure::Error> {
        for step_number in 1..=self.cycles_amount {
            log::info!("Starting funds rotation cycle {}", step_number);

            self.funds_rotation_step().await?;
        }

        Ok(())
    }

    /// Transfers the money between intermediate accounts. For each account with
    /// ID `N`, money are transferred to the account with ID `N + 1`.
    async fn funds_rotation_step(&mut self) -> Result<(), failure::Error> {
        let mut signed_transfers = Vec::with_capacity(self.n_accounts);

        for from_id in 0..self.n_accounts {
            let from_acc = &self.accounts[from_id];
            let to_id = self.acc_for_transfer(from_id);
            let to_acc = &self.accounts[to_id];

            let fee = self.transfer_fee(&to_acc).await;
            let transfer = self
                .sign_transfer(from_acc, to_acc, self.transfer_size.clone(), fee)
                .await;

            signed_transfers.push(transfer);
        }

        log::info!("Signed transfers, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        let txs_chunks = DynamicChunks::new(signed_transfers, &self.block_sizes);
        for tx_batch in txs_chunks {
            let mut tx_futures = vec![];
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_future = self.provider.send_tx(tx.clone(), eth_sign.clone());

                tx_futures.push(tx_future);
            }
            let mut sent_txs = SentTransactions::new();
            sent_txs.tx_hashes = try_join_all(tx_futures).await?;

            let sent_txs_amount = sent_txs.len();
            verified += sent_txs_amount;

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, self.verify_timeout, &self.provider).await?;

            log::info!(
                "Sent and verified {}/{} txs ({} on this iteration)",
                verified,
                to_verify,
                sent_txs_amount
            );
        }

        log::info!("Transfers are sent and verified");

        Ok(())
    }

    /// Transfers all the money from the intermediate accounts back to the main account.
    async fn collect_funds(&mut self) -> Result<(), failure::Error> {
        log::info!("Starting collecting funds back to the main account");

        let mut signed_transfers = Vec::with_capacity(self.n_accounts);

        for from_id in 0..self.n_accounts {
            let from_acc = &self.accounts[from_id];
            let to_acc = &self.main_wallet;

            let fee = self.transfer_fee(&to_acc).await;

            let comitted_account_state = self
                .provider
                .account_info(from_acc.address())
                .await?
                .committed;
            let account_balance = comitted_account_state.balances[TestWallet::TOKEN_NAME]
                .0
                .clone();
            let transfer_amount = &account_balance - &fee;
            let transfer_amount = closest_packable_token_amount(&transfer_amount);
            let transfer = self
                .sign_transfer(from_acc, to_acc, transfer_amount, fee)
                .await;

            signed_transfers.push(transfer);
        }

        log::info!("Signed transfers, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        let txs_chunks = DynamicChunks::new(signed_transfers, &self.block_sizes);
        for tx_batch in txs_chunks {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self.provider.send_tx(tx.clone(), eth_sign.clone()).await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            let sent_txs_amount = sent_txs.len();
            verified += sent_txs_amount;

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, self.verify_timeout, &self.provider).await?;

            log::info!(
                "Sent and verified {}/{} txs ({} on this iteration)",
                verified,
                to_verify,
                sent_txs_amount
            );
        }

        log::info!("Collecting funds completed");
        Ok(())
    }

    /// Withdraws the money from the main account back to the Ethereum.
    async fn withdraw(&mut self) -> Result<(), failure::Error> {
        let current_balance = self.main_wallet.eth_provider.balance().await?;

        let fee = self.withdraw_fee(&self.main_wallet).await;

        let comitted_account_state = self
            .provider
            .account_info(self.main_wallet.address())
            .await?
            .committed;
        let account_balance = comitted_account_state.balances[TestWallet::TOKEN_NAME]
            .0
            .clone();
        let withdraw_amount = &account_balance - &fee;
        let withdraw_amount = closest_packable_token_amount(&withdraw_amount);

        log::info!(
            "Starting withdrawing phase. Withdrawing {} ETH back to the Ethereum",
            format_ether(&withdraw_amount)
        );

        let (tx, eth_sign) = self
            .main_wallet
            .sign_withdraw(withdraw_amount.clone(), fee)
            .await?;
        let tx_hash = self.provider.send_tx(tx.clone(), eth_sign.clone()).await?;
        let mut sent_txs = SentTransactions::new();
        sent_txs.add_tx_hash(tx_hash);

        wait_for_verify(sent_txs, self.verify_timeout, &self.provider).await?;

        log::info!("Withdrawing funds completed");

        self.wait_for_eth_balance(current_balance, withdraw_amount)
            .await?;

        Ok(())
    }

    async fn finish(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }

    /// Waits for main ETH account to receive funds on its balance.
    /// Returns an error if funds are not received within a reasonable amount of time.
    async fn wait_for_eth_balance(
        &self,
        current_balance: BigUint,
        withdraw_amount: BigUint,
    ) -> Result<(), failure::Error> {
        log::info!("Awaiting for ETH funds to be received");

        let expected_balance = current_balance + withdraw_amount;

        let timeout_minutes = 10;
        let timeout = Duration::from_secs(timeout_minutes * 60);
        let start = Instant::now();

        let polling_interval = Duration::from_millis(250);
        let mut timer = time::interval(polling_interval);

        loop {
            let current_balance = self.main_wallet.eth_provider.balance().await?;
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

        log::info!("ETH funds received");
        Ok(())
    }

    /// Obtains a fee required for the transfer operation.
    async fn transfer_fee(&self, to: &TestWallet) -> BigUint {
        let fee = self
            .provider
            .get_tx_fee(TxFeeTypes::Transfer, to.address(), TestWallet::TOKEN_NAME)
            .await
            .expect("Can't get tx fee")
            .total_fee;

        closest_packable_fee_amount(&fee)
    }

    /// Obtains a fee required for the withdraw operation.
    async fn withdraw_fee(&self, to: &TestWallet) -> BigUint {
        let fee = self
            .provider
            .get_tx_fee(TxFeeTypes::Withdraw, to.address(), TestWallet::TOKEN_NAME)
            .await
            .expect("Can't get tx fee")
            .total_fee;

        closest_packable_fee_amount(&fee)
    }

    /// Creates a signed transfer transaction.
    /// Sender and receiver are chosen from the generated
    /// accounts, determined by its indices.
    async fn sign_transfer(
        &self,
        from: &TestWallet,
        to: &TestWallet,
        amount: impl Into<BigUint>,
        fee: impl Into<BigUint>,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        from.sign_transfer(to.address(), amount, fee).await.unwrap()
    }

    /// Generates an ID for funds transfer. The ID is the ID of the next
    /// account, treating the accounts array like a circle buffer:
    /// given 3 accounts, IDs returned for queries (0, 1, 2) will be
    /// (1, 2, 0) correspondingly.
    fn acc_for_transfer(&self, from_idx: usize) -> usize {
        (from_idx + 1) % self.accounts.len()
    }

    /// Load block sizes to use in test for generated blocks.
    /// This method assumes that loadtest and server share the same env config,
    /// since the value is loaded from the env.
    fn get_block_sizes(use_all_block_sizes: bool) -> Vec<usize> {
        let options = ConfigurationOptions::from_env();
        if use_all_block_sizes {
            // Load all the supported block sizes.
            options.available_block_chunk_sizes
        } else {
            // Use only the max block size (for more quick execution).
            let max_size = *options.available_block_chunk_sizes.iter().max().unwrap();

            vec![max_size]
        }
    }
}

/// Runs the real-life test scenario.
/// For description, see the module doc-comment.
pub fn run_scenario(mut ctx: ScenarioContext) {
    let provider = Provider::new(Network::Localhost);

    let mut scenario = ScenarioExecutor::new(&mut ctx, provider);

    // Run the scenario.
    log::info!("Starting the real-life test");
    ctx.rt.block_on(scenario.run());
}
