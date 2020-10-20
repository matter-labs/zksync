// Built-in uses
use std::{collections::BTreeMap, fmt::Debug};
// External uses
use futures::{future::BoxFuture, FutureExt};
use num::BigUint;
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync::{types::BlockStatus, utils::closest_packable_token_amount, Provider};
use zksync_config::ConfigurationOptions;
use zksync_utils::format_ether;
// Local uses
use crate::{
    api::CancellationToken,
    config::Config,
    journal::Journal,
    monitor::Monitor,
    scenarios::Scenario,
    test_wallet::TestWallet,
    utils::{try_wait_all_failsafe, wait_all},
    FiveSummaryStats,
};

type ApiTestsFuture = BoxFuture<'static, anyhow::Result<BTreeMap<String, FiveSummaryStats>>>;

/// Full report with the results of loadtest execution.
///
/// This report contains two major types: scenarios with transactions and API requests.
#[derive(Debug, Serialize, Deserialize)]
pub struct Report {
    /// Scenarios report.
    pub scenarios: BTreeMap<String, FiveSummaryStats>,
    /// API requests report.
    pub api: BTreeMap<String, FiveSummaryStats>,
}

/// Executor of load tests.

/// In parallel, it exeuctes scenarios with transactions and performs load tests of the API.
///
/// During the scenarios execution, it uses the information from the `AccountInfo` to create
/// a bunch of wallets and distribute the funds needed to perform transactions execution
/// between them. Upon completion, the remaining balances are returned to the main wallet.
pub struct LoadtestExecutor {
    /// Main account to deposit ETH from / return ETH back to.
    main_wallet: TestWallet,
    monitor: Monitor,
    eth_options: ConfigurationOptions,
    /// Estimated fee amount for any zkSync operation.
    sufficient_fee: BigUint,
    scenarios: Vec<(Box<dyn Scenario>, Vec<TestWallet>)>,
    api_tests: Option<(ApiTestsFuture, CancellationToken)>,
}

impl LoadtestExecutor {
    /// Creates a new executor instance.
    pub async fn new(config: Config, eth_options: ConfigurationOptions) -> anyhow::Result<Self> {
        let monitor = Monitor::new(Provider::new(config.network.name)).await;

        log::info!("Creating scenarios...");

        let scenarios = config
            .scenarios
            .into_iter()
            .map(|cfg| (cfg.into_scenario(), Vec::new()))
            .collect();

        // Create main account to deposit money from and to return money back later.
        let main_wallet =
            TestWallet::from_info(monitor.clone(), &config.main_wallet, &eth_options).await;
        let sufficient_fee = main_wallet.sufficient_fee().await?;

        log::info!("Fee is {}", format_ether(&sufficient_fee));

        // TODO Use one of random wallets from the preparation step.
        let (api_tests, cancel) = crate::api::run(
            monitor.clone(),
            TestWallet::from_info(monitor.clone(), &config.main_wallet, &eth_options)
                .await
                .into_inner(),
        );

        Ok(Self {
            monitor,
            eth_options,
            main_wallet,
            scenarios,
            sufficient_fee,
            api_tests: Some((api_tests.boxed(), cancel)),
        })
    }

    /// Performs configured loadtests routine.
    pub async fn run(mut self) -> anyhow::Result<Report> {
        // Preliminary steps for creating wallets with funds.
        self.prepare().await?;
        // Spawn an additional loadtest routine with a lot of API requests.
        let (api_tests, token) = self.api_tests.take().unwrap();
        let api_handle = tokio::spawn(api_tests);
        // Launch the main loadtest routine.
        let journal = self.process().await?;
        // Stop API loadtest routine
        token.cancel();
        // Refund remaining funds to the main wallet.
        self.refund().await?;

        Ok(Report {
            scenarios: journal.five_stats_summary()?,
            api: api_handle.await??,
        })
    }

    /// Makes initial deposit to the main wallet.
    async fn prepare(&mut self) -> anyhow::Result<()> {
        // Create requested wallets and make initial deposit with the sufficient amount.
        let resources = self
            .scenarios
            .iter()
            .map(|x| x.0.requested_resources(&self.sufficient_fee));

        // Create intermediate wallets and compute total amount to deposit and needed
        // balances for wallets.
        let mut amount_to_deposit = BigUint::from(0_u64);
        let mut wallets = Vec::new();
        for resource in resources {
            let wallet_balance = closest_packable_token_amount(
                &(&resource.balance_per_wallet + BigUint::from(2_u64) * &self.sufficient_fee),
            );
            let scenario_amount = closest_packable_token_amount(
                &(resource.wallets_amount * &wallet_balance + &self.sufficient_fee),
            );

            // TODO Compute sufficient balance accurate.
            amount_to_deposit += scenario_amount * BigUint::from(2_u64);

            let scenario_wallets = wait_all((0..resource.wallets_amount).map(|_| {
                TestWallet::new_random(
                    self.main_wallet.token_name().clone(),
                    self.monitor.clone(),
                    &self.eth_options,
                )
            }))
            .await;

            wallets.push((scenario_wallets, wallet_balance));
        }

        // Make deposit from Ethereum network to the zkSync one.
        let amount_to_deposit = closest_packable_token_amount(&amount_to_deposit);
        let eth_balance = self.main_wallet.eth_balance().await?;
        anyhow::ensure!(
            eth_balance > amount_to_deposit,
            "Not enough balance in the main wallet to perform this test, actual: {}, expected: {}",
            format_ether(&eth_balance),
            format_ether(&amount_to_deposit),
        );

        log::info!(
            "Deposit {} for main wallet",
            format_ether(&amount_to_deposit),
        );

        let priority_op = self.main_wallet.deposit(amount_to_deposit).await.unwrap();
        self.monitor
            .wait_for_priority_op(BlockStatus::Committed, &priority_op)
            .await?;

        // Now when deposits are done it is time to update account id.
        self.main_wallet.update_account_id().await?;
        assert!(
            self.main_wallet.account_id().is_some(),
            "Account ID was not set after deposit for main account"
        );

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (tx, sign) = self
            .main_wallet
            .sign_change_pubkey(self.sufficient_fee.clone())
            .await?;
        let tx_hash = self.monitor.send_tx(tx, sign).await?;
        self.monitor
            .wait_for_tx(BlockStatus::Committed, tx_hash)
            .await?;

        log::info!("Deposit phase completed");

        // Split the money from the main account between the intermediate wallets.
        for (scenario_index, (mut scenario_wallets, scenario_amount)) in
            wallets.into_iter().enumerate()
        {
            log::info!(
                "Preparing transactions for the initial transfer for `{}` scenario: \
                {} to will be send to each of {} new wallets",
                self.scenarios[scenario_index].0,
                format_ether(&scenario_amount),
                scenario_wallets.len()
            );

            let mut tx_hashes = Vec::new();
            for wallet in &scenario_wallets {
                let (tx, sign) = self
                    .main_wallet
                    .sign_transfer(
                        wallet.address(),
                        scenario_amount.clone(),
                        self.sufficient_fee.clone(),
                    )
                    .await?;
                tx_hashes.push(self.monitor.send_tx(tx, sign).await?);
            }

            try_wait_all_failsafe(
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| self.monitor.wait_for_tx(BlockStatus::Committed, tx_hash)),
            )
            .await?;

            log::info!(
                "All the initial transfers for `{}` scenario have been committed.",
                self.scenarios[scenario_index].0,
            );

            let mut tx_hashes = Vec::new();
            for wallet in &mut scenario_wallets {
                wallet.update_account_id().await?;
                assert!(
                    wallet.account_id().is_some(),
                    "Account ID was not set after deposit for the account"
                );

                let (tx, sign) = wallet
                    .sign_change_pubkey(self.sufficient_fee.clone())
                    .await?;
                tx_hashes.push(self.monitor.send_tx(tx, sign).await?);
            }

            try_wait_all_failsafe(
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| self.monitor.wait_for_tx(BlockStatus::Committed, tx_hash)),
            )
            .await?;

            let scenario_handle = &mut self.scenarios[scenario_index];
            scenario_handle
                .0
                .prepare(&self.monitor, &self.sufficient_fee, &scenario_wallets)
                .await?;
            scenario_handle.1 = scenario_wallets;
        }

        log::info!("Awaiting for pending tasks verification...",);
        self.monitor.wait_for_verify().await;
        Ok(())
    }

    /// Performs main step of the load tests.
    async fn process(&mut self) -> anyhow::Result<Journal> {
        log::info!("Starting TPS measuring...");
        let monitor = self.monitor.clone();
        monitor.start().await;

        // Run scenarios concurrently.
        let sufficient_fee = self.sufficient_fee.clone();
        try_wait_all_failsafe(
            self.scenarios
                .iter_mut()
                .map(|(scenario, wallets)| scenario.run(&monitor, &sufficient_fee, wallets)),
        )
        .await?;
        self.monitor.wait_for_verify().await;

        let logs = self.monitor.finish().await;
        log::info!("TPS measuring finished...");
        Ok(logs)
    }

    /// Returns the remaining funds to the main wallet.
    async fn refund(&mut self) -> anyhow::Result<()> {
        log::info!("Refunding the remaining tokens to the main wallet.");

        // Transfer the remaining balances of the intermediate wallets into the main one.
        for (scenario, scenario_wallets) in &mut self.scenarios {
            let monitor = self.monitor.clone();
            let sufficient_fee = self.sufficient_fee.clone();
            scenario
                .finalize(&monitor, &sufficient_fee, &scenario_wallets)
                .await?;

            let main_address = self.main_wallet.address();
            let txs_queue = try_wait_all_failsafe(scenario_wallets.iter().map(|wallet| {
                let sufficient_fee = sufficient_fee.clone();
                async move {
                    let balance = wallet.balance(BlockStatus::Verified).await?;
                    let withdraw_amount =
                        closest_packable_token_amount(&(balance - &sufficient_fee));

                    wallet
                        .sign_transfer(
                            main_address,
                            withdraw_amount.clone(),
                            sufficient_fee.clone(),
                        )
                        .await
                }
            }))
            .await?;

            let tx_hashes = try_wait_all_failsafe(
                txs_queue
                    .into_iter()
                    .map(|(tx, sign)| monitor.send_tx(tx, sign)),
            )
            .await?;

            try_wait_all_failsafe(
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| monitor.wait_for_tx(BlockStatus::Committed, tx_hash)),
            )
            .await?;
        }

        // Withdraw remaining balance from the zkSync network back to the Ethereum one.
        let main_wallet_balance = self.main_wallet.balance(BlockStatus::Committed).await?;
        if main_wallet_balance > self.sufficient_fee {
            log::info!(
                "Main wallet has {} balance, making refund...",
                format_ether(&main_wallet_balance)
            );

            let withdraw_amount =
                closest_packable_token_amount(&(main_wallet_balance - &self.sufficient_fee));
            let (tx, sign) = self
                .main_wallet
                .sign_withdraw(withdraw_amount, self.sufficient_fee.clone())
                .await?;
            self.monitor.send_tx(tx, sign).await?;
        }

        self.monitor.wait_for_verify().await;

        Ok(())
    }
}
