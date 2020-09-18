//! API test is a scenario performing verification of
//! zkSync server API responses correctness.
//!
//! Basically, it's an integration test built on a base
//! of the `loadtest`, as this crate has convenient tools
//! for quick and robust implementation.
//!
//! Later this test should be replaced with unit-tests in
//! the server crate itself, but currently the server API
//! infrastructure is a bit hostile for unit-testing, and
//! creating an integration test will be more fluent.

// Built-in deps
use std::time::Duration;
// External deps
use num::BigUint;
use web3::transports::{EventLoopHandle, Http};
// Workspace deps
use models::node::{closest_packable_fee_amount, tx::PackedEthSignature, FranklinTx};
use testkit::zksync_account::ZksyncAccount;
use zksync_utils::format_ether;
// Local deps
use self::submit_tx::SubmitTxTester;
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

mod submit_tx;

#[derive(Debug)]
pub struct TestExecutor {
    rpc_client: RpcClient,

    /// Main account to be used in test.
    main_account: TestAccount,

    /// Amount of time to wait for one zkSync block to be verified.
    verify_timeout: Duration,

    /// Estimated fee amount for any zkSync operation.
    estimated_fee_for_op: BigUint,

    /// Event loop handle so transport for Eth account won't be invalidated.
    _event_loop_handle: EventLoopHandle,
}

impl TestExecutor {
    /// Creates a real-life scenario executor.
    pub fn new(ctx: &ScenarioContext, rpc_client: RpcClient) -> Self {
        // Load the config for the test from JSON file.
        let config = RealLifeConfig::load(&ctx.config_path);

        // Create a transport for Ethereum account.
        let (_event_loop_handle, transport) =
            Http::new(&ctx.options.web3_url).expect("http transport start");

        // Create main account to deposit money from and to return money back later.
        let main_account = TestAccount::from_info(&config.input_account, &transport, &ctx.options);

        Self {
            rpc_client,
            main_account,
            verify_timeout: Duration::from_secs(config.block_timeout),
            estimated_fee_for_op: 0u32.into(),

            _event_loop_handle,
        }
    }

    /// Runs the test closure surrounding it with the log entries.
    ///
    /// Sample usage:
    ///
    /// ```rust, ignore
    /// TestExecutor::execute_test("Test name", || some_async_fn()).await;
    /// ```
    ///
    /// This will result in the following lines in the log:
    ///
    /// ```text
    /// Running test: "Test name"
    /// Test "Test name": OK
    /// ```
    pub async fn execute_test<F, O>(test_name: &str, test: F)
    where
        F: FnOnce() -> O,
        O: std::future::Future<Output = ()>,
    {
        log::info!("Running test: \"{}\"", test_name);

        test().await;

        log::info!("Test \"{}\": OK", test_name);
    }

    /// Infallible test runner.
    pub async fn run(&mut self) {
        if let Err(error) = self.run_test().await {
            log::error!("API test erred with the following error: {}", error);
        } else {
            log::info!("API test completed successfully");
        }
    }

    /// Runs the test step-by-step. Every test step is encapsulated into its own function.
    pub async fn run_test(&mut self) -> Result<(), failure::Error> {
        self.initialize().await?;
        self.deposit().await?;

        // Actual test runners.
        SubmitTxTester::new(self).run().await?;

        self.finish().await?;

        Ok(())
    }

    /// Initializes the test, preparing the main account for the interaction.
    async fn initialize(&mut self) -> Result<(), failure::Error> {
        // First of all, we have to update both the Ethereum and ZKSync accounts nonce values.
        self.main_account
            .update_nonce_values(&self.rpc_client)
            .await?;

        // Then, we have to get the fee value (assuming that dev-ticker is used, we estimate
        // the fee in such a way that it will always be sufficient).
        // Withdraw operation has more chunks, so we estimate fee for it.
        let mut fee = self.withdraw_fee(&self.main_account.zk_acc).await;

        // To be sure that we will have enough funds for all the transfers,
        // we will request 1.2x of the suggested fees. All the unspent funds
        // will be withdrawn later.
        fee = fee * BigUint::from(120u32) / BigUint::from(100u32);

        // And after that we have to make the fee packable.
        fee = closest_packable_fee_amount(&fee);

        self.estimated_fee_for_op = fee;

        Ok(())
    }

    /// Runs the initial deposit of the money onto the main account.
    async fn deposit(&mut self) -> Result<(), failure::Error> {
        let mut amount_to_deposit = 100u32.into();
        amount_to_deposit += self.estimated_fee_for_op.clone();

        let account_balance = self.main_account.eth_acc.eth_balance().await?;
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

    /// Obtains a fee required for the withdraw operation.
    async fn withdraw_fee(&self, to_acc: &ZksyncAccount) -> BigUint {
        let fee = self
            .rpc_client
            .get_tx_fee("Withdraw", to_acc.address, "ETH")
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

    let mut scenario = TestExecutor::new(&ctx, rpc_client);

    // Run the scenario.
    log::info!("Starting the API integration test");
    ctx.rt.block_on(scenario.run());
}
