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
use std::time::{Duration, Instant};
// External deps
use bigdecimal::BigDecimal;
use chrono::Utc;
use tokio::{fs, time};
use web3::transports::{EventLoopHandle, Http};
// Workspace deps
use models::{
    config_options::ConfigurationOptions,
    node::{tx::PackedEthSignature, FranklinTx},
};
use testkit::zksync_account::ZksyncAccount;
// Local deps
use crate::{
    rpc_client::RpcClient,
    scenarios::{
        configs::RealLifeConfig,
        utils::{deposit_single, wait_for_verify},
        ScenarioContext,
    },
    sent_transactions::SentTransactions,
    test_accounts::TestAccount,
};

#[derive(Debug)]
struct ScenarioExecutor {
    rpc_client: RpcClient,

    /// Main account to deposit ETH from / return ETH back to.
    main_account: TestAccount,

    /// Intermediate account to rotate funds within.
    accounts: Vec<ZksyncAccount>,

    /// Amount of intermediate accounts.
    n_accounts: usize,
    /// Transfer amount per accounts (in wei).
    transfer_size: u64,
    /// Amount of cycles for funds rotation.
    cycles_amount: u32,

    /// Biggest supported block size (to not overload the node
    /// with too many txs at the moment)
    max_block_size: usize,

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
        let accounts = (0..config.n_accounts)
            .map(|_| ZksyncAccount::rand())
            .collect();

        // Create a transport for Ethereum account.
        let (_event_loop_handle, transport) =
            Http::new(&ctx.options.web3_url).expect("http transport start");

        // Create main account to deposit money from and to return money back later.
        let main_account = TestAccount::from_info(&config.input_account, &transport, &ctx.options);

        Self {
            rpc_client,

            main_account,
            accounts,

            n_accounts: config.n_accounts,
            transfer_size: config.transfer_size,
            cycles_amount: config.cycles_amount,
            max_block_size: Self::get_max_supported_block_size(),

            verify_timeout: Duration::from_secs(config.block_timeout),

            _event_loop_handle,
        }
    }

    /// Infallible test runner which performs the emergency exit if any step of the test
    /// fails.
    pub async fn run(&mut self) {
        if let Err(error) = self.run_test().await {
            log::error!("Loadtest erred with the following error: {}", error);
            log::warn!("Performing the emergency exit");
            self.emergency_exit().await;
        } else {
            log::info!("Loadtest completed successfully");
        }
    }

    /// Method to be used if the scenario will fail on the any step.
    /// It stores all the zkSync account keys into a file named
    /// like "emergency_output_2020_05_05_12_23_55.txt"
    /// so the funds left on accounts will not be lost.
    ///
    /// If saving the file fails, the accounts are printed to the log.
    async fn emergency_exit(&self) {
        // Timestamp is used to generate unique file name postfix.
        let timestamp = Utc::now();
        let timestamp_str = timestamp.format("%Y_%m_%d_%H_%M_%S").to_string();

        let output_file_name = format!("emergency_output_{}.txt", timestamp_str);

        let mut account_list = String::new();

        // Add all the accounts to the string.
        // Debug representations of account contains both zkSync and Ethereum private keys.
        account_list += &format!("{:?}\n", self.main_account.zk_acc);
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
        self.initialize().await?;
        self.deposit().await?;
        self.initial_transfer().await?;
        self.funds_rotation().await?;
        self.collect_funds().await?;
        self.withdraw().await?;
        self.finish().await?;

        Ok(())
    }

    /// Initializes the test, preparing the main account for the interaction.
    async fn initialize(&mut self) -> Result<(), failure::Error> {
        // First of all, we have to update both the Ethereum and ZKSync accounts nonce values.
        self.main_account
            .update_nonce_values(&self.rpc_client)
            .await?;

        Ok(())
    }

    /// Runs the initial deposit of the money onto the main account.
    async fn deposit(&mut self) -> Result<(), failure::Error> {
        // Amount of money we need to deposit.
        // Initialize it with the raw amount: only sum of transfers per account.
        // Fees will be set to zero, so there is no need in any additional funds.
        let amount_to_deposit =
            BigDecimal::from(self.transfer_size) * BigDecimal::from(self.n_accounts as u64);

        let account_balance = self.main_account.eth_acc.eth_balance().await?;
        log::info!("Main account ETH balance: {}", account_balance);

        // Ensure that account does have enough money.
        if amount_to_deposit > account_balance {
            panic!("Main ETH account does not have enough balance to run the test with the provided config");
        }

        log::info!(
            "Starting depositing phase. Depositing {} wei to the main account",
            amount_to_deposit
        );

        // Deposit funds and wait for operation to be executed.
        deposit_single(&self.main_account, amount_to_deposit, &self.rpc_client).await?;

        log::info!("Deposit sent and verified");

        // Now when deposits are done it is time to update account id.
        self.main_account
            .update_account_id(&self.rpc_client)
            .await?;

        log::info!("Main account ID set");

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (change_pubkey_tx, eth_sign) = (self.main_account.sign_change_pubkey(), None);
        let mut sent_txs = SentTransactions::new();
        let tx_hash = self.rpc_client.send_tx(change_pubkey_tx, eth_sign).await?;
        sent_txs.add_tx_hash(tx_hash);
        wait_for_verify(sent_txs, self.verify_timeout, &self.rpc_client).await?;

        log::info!("Main account pubkey changed");

        log::info!("Deposit phase completed");

        Ok(())
    }

    /// Splits the money from the main account between the intermediate accounts
    /// with the `TransferToNew` operations.
    async fn initial_transfer(&mut self) -> Result<(), failure::Error> {
        log::info!(
            "Starting initial transfer. {} wei will be send to each of {} new accounts",
            self.transfer_size,
            self.n_accounts
        );

        let signed_transfers: Vec<_> = (0..self.n_accounts)
            .map(|to_idx| {
                let from_acc = &self.main_account.zk_acc;
                let to_acc = &self.accounts[to_idx];
                self.sign_transfer(from_acc, to_acc, self.transfer_size)
            })
            .collect();

        log::info!("Signed all the initial transfer transactions, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        for tx_batch in signed_transfers.chunks(self.max_block_size) {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self
                    .rpc_client
                    .send_tx(tx.clone(), eth_sign.clone())
                    .await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            verified += sent_txs.len();

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, self.verify_timeout, &self.rpc_client).await?;

            log::info!("Sent and verified {}/{} txs", verified, to_verify);
        }

        log::info!("All the initial transfers are completed");
        log::info!("Updating the accounts info and changing their public keys");

        // After all the initial transfer completed, we have to update new account IDs
        // and change public keys of accounts (so we'll be able to send transfers from them).
        let mut sent_txs = SentTransactions::new();
        for account in self.accounts.iter() {
            let resp = self
                .rpc_client
                .account_state_info(account.address)
                .await
                .expect("rpc error");
            assert!(resp.id.is_some(), "Account ID is none for new account");
            account.set_account_id(resp.id);

            let change_pubkey_tx = FranklinTx::ChangePubKey(Box::new(
                account.create_change_pubkey_tx(None, true, false),
            ));

            let tx_hash = self.rpc_client.send_tx(change_pubkey_tx, None).await?;
            sent_txs.add_tx_hash(tx_hash);
        }
        // Calculate the estimated amount of blocks for all the txs to be processed.
        let n_blocks = (self.accounts.len() / self.max_block_size + 1) as u32;
        wait_for_verify(sent_txs, self.verify_timeout * n_blocks, &self.rpc_client).await?;

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
        let signed_transfers: Vec<_> = (0..self.n_accounts)
            .map(|from_id| {
                let from_acc = &self.accounts[from_id];
                let to_id = self.acc_for_transfer(from_id);
                let to_acc = &self.accounts[to_id];
                self.sign_transfer(from_acc, to_acc, self.transfer_size)
            })
            .collect();

        log::info!("Signed transfers, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        for tx_batch in signed_transfers.chunks(self.max_block_size) {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self
                    .rpc_client
                    .send_tx(tx.clone(), eth_sign.clone())
                    .await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            verified += sent_txs.len();

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, self.verify_timeout, &self.rpc_client).await?;

            log::info!("Sent and verified {}/{} txs", verified, to_verify);
        }

        log::info!("Transfers are sent and verified");

        Ok(())
    }

    /// Transfers all the money from the intermediate accounts back to the main account.
    async fn collect_funds(&mut self) -> Result<(), failure::Error> {
        log::info!("Starting collecting funds back to the main account",);

        let signed_transfers: Vec<_> = (0..self.n_accounts)
            .map(|from_id| {
                let from_acc = &self.accounts[from_id];
                let to_acc = &self.main_account.zk_acc;
                self.sign_transfer(from_acc, to_acc, self.transfer_size)
            })
            .collect();

        log::info!("Signed transfers, sending");

        // Send txs by batches that can fit in one block.
        let to_verify = signed_transfers.len();
        let mut verified = 0;
        for tx_batch in signed_transfers.chunks(self.max_block_size) {
            let mut sent_txs = SentTransactions::new();
            // Send each tx.
            for (tx, eth_sign) in tx_batch {
                let tx_hash = self
                    .rpc_client
                    .send_tx(tx.clone(), eth_sign.clone())
                    .await?;
                sent_txs.add_tx_hash(tx_hash);
            }

            verified += sent_txs.len();

            // Wait until all the transactions are verified.
            wait_for_verify(sent_txs, self.verify_timeout, &self.rpc_client).await?;

            log::info!("Sent and verified {}/{} txs", verified, to_verify);
        }

        log::info!("Collecting funds completed");
        Ok(())
    }

    /// Withdraws the money from the main account back to the Ethereum.
    async fn withdraw(&mut self) -> Result<(), failure::Error> {
        let mut sent_txs = SentTransactions::new();

        let amount_to_withdraw =
            BigDecimal::from(self.transfer_size) * BigDecimal::from(self.n_accounts as u64);

        let current_balance = self.main_account.eth_acc.eth_balance().await?;

        log::info!(
            "Starting withdrawing phase. Withdrawing {} wei back to the Ethereum",
            amount_to_withdraw
        );

        let (tx, eth_sign) = self
            .main_account
            .sign_withdraw_single(amount_to_withdraw.clone());
        let tx_hash = self
            .rpc_client
            .send_tx(tx.clone(), eth_sign.clone())
            .await?;
        sent_txs.add_tx_hash(tx_hash);

        wait_for_verify(sent_txs, self.verify_timeout, &self.rpc_client).await?;

        log::info!("Withdrawing funds completed");

        self.wait_for_eth_balance(current_balance, amount_to_withdraw)
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
        current_balance: BigDecimal,
        withdraw_amount: BigDecimal,
    ) -> Result<(), failure::Error> {
        log::info!("Awaiting for ETH funds to be received");

        let expected_balance = current_balance + withdraw_amount;

        let timeout_minutes = 10;
        let timeout = Duration::from_secs(timeout_minutes * 60);
        let start = Instant::now();

        let polling_interval = Duration::from_millis(250);
        let mut timer = time::interval(polling_interval);

        loop {
            let current_balance = self.main_account.eth_acc.eth_balance().await?;
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

    /// Creates a signed transfer transaction.
    /// Sender and receiver are chosen from the generated
    /// accounts, determined by its indices.
    fn sign_transfer(
        &self,
        from: &ZksyncAccount,
        to: &ZksyncAccount,
        amount: u64,
    ) -> (FranklinTx, Option<PackedEthSignature>) {
        let (tx, eth_signature) = from.sign_transfer(
            0, // ETH
            "ETH",
            BigDecimal::from(amount),
            BigDecimal::from(0),
            &to.address,
            None,
            true,
        );

        (FranklinTx::Transfer(Box::new(tx)), Some(eth_signature))
    }

    /// Generates an ID for funds transfer. The ID is the ID of the next
    /// account, treating the accounts array like a circle buffer:
    /// given 3 accounts, IDs returned for queries (0, 1, 2) will be
    /// (1, 2, 0) correspondingly.
    fn acc_for_transfer(&self, from_idx: usize) -> usize {
        (from_idx + 1) % self.accounts.len()
    }

    /// Loads the biggest supported block size.
    /// This method assumes that loadtest and server share the same env config,
    /// since the value is loaded from the env.
    fn get_max_supported_block_size() -> usize {
        let options = ConfigurationOptions::from_env();

        *options.available_block_chunk_sizes.iter().max().unwrap()
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
