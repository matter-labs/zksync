// Built-in uses
use std::{collections::BTreeMap, fmt::Debug};
// External uses
use num::BigUint;
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync::{
    types::BlockStatus,
    utils::{closest_packable_fee_amount, closest_packable_token_amount},
    Provider,
};
use zksync_config::ConfigurationOptions;
use zksync_utils::format_ether;
// Local uses
use crate::{
    api::{self, ApiTestsFuture, ApiTestsReport, CancellationToken},
    config::{Config, NetworkConfig},
    journal::Journal,
    monitor::Monitor,
    scenarios::{Fees, Scenario, ScenariosTestsReport},
    test_wallet::TestWallet,
    utils::{
        gwei_to_wei, wait_all_chunks, wait_all_failsafe, wait_all_failsafe_chunks, CHUNK_SIZES,
    },
};

/// Full report with the results of loadtest execution.
///
/// This report contains two major types: scenarios with transactions and API requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    /// Scenarios report.
    pub scenarios: ScenariosTestsReport,
    /// API requests report.
    pub api: BTreeMap<String, ApiTestsReport>,
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
    env_options: ConfigurationOptions,
    /// Estimated fee amount for any zkSync operation.
    fees: Fees,
    scenarios: Vec<(Box<dyn Scenario>, Vec<TestWallet>)>,
    api_tests: Option<(ApiTestsFuture, CancellationToken)>,
}

impl Fees {
    fn from_config(config: &NetworkConfig, default_fee: BigUint) -> Self {
        Self {
            eth: closest_packable_fee_amount(
                &config
                    .eth_fee
                    .map(gwei_to_wei)
                    .unwrap_or_else(|| &default_fee * BigUint::from(10_u64)),
            ),
            zksync: closest_packable_fee_amount(
                &config.zksync_fee.map(gwei_to_wei).unwrap_or(default_fee),
            ),
        }
    }
}

impl LoadtestExecutor {
    /// Creates a new executor instance.
    pub async fn new(config: Config, env_options: ConfigurationOptions) -> anyhow::Result<Self> {
        let monitor = Monitor::new(Provider::new(config.network.name)).await;

        log::info!("Creating scenarios...");

        let scenarios = config
            .scenarios
            .into_iter()
            .map(|cfg| (cfg.into_scenario(), Vec::new()))
            .collect();

        // Create main account to deposit money from and to return money back later.
        let main_wallet =
            TestWallet::from_info(monitor.clone(), &config.main_wallet, &env_options).await;

        let default_fee = main_wallet.sufficient_fee().await?;
        let fees = Fees::from_config(&config.network, default_fee);

        log::info!("Eth fee is {}", format_ether(&fees.eth));
        log::info!("zkSync fee is {}", format_ether(&fees.zksync));

        let api_tests = api::run(monitor.clone());

        Ok(Self {
            monitor,
            env_options,
            main_wallet,
            scenarios,
            fees,
            api_tests: Some(api_tests),
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
            scenarios: journal.report(),
            api: api_handle.await?,
        })
    }

    /// Makes initial deposit to the main wallet.
    async fn prepare(&mut self) -> anyhow::Result<()> {
        // Create requested wallets and make initial deposit with the sufficient amount.
        let resources = self
            .scenarios
            .iter()
            .map(|x| x.0.requested_resources(&self.fees));

        // Create intermediate wallets and compute total amount to deposit and needed
        // balances for wallets.
        let mut amount_to_deposit = &self.fees.eth + &self.fees.zksync * BigUint::from(10_u64);
        let mut wallets = Vec::new();
        for resource in resources {
            let wallet_balance = closest_packable_token_amount(
                &(&resource.balance_per_wallet + BigUint::from(5_u64) * &self.fees.zksync),
            );

            let scenario_amount = BigUint::from(resource.wallets_amount) * &wallet_balance
                + BigUint::from(10_u64) * &self.fees.zksync
                + &self.fees.eth;
            amount_to_deposit += scenario_amount;

            let scenario_wallets = wait_all_chunks(
                CHUNK_SIZES,
                (0..resource.wallets_amount).map(|_| {
                    TestWallet::new_random(
                        self.main_wallet.token_name().clone(),
                        self.monitor.clone(),
                        &self.env_options,
                    )
                }),
            )
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

        let priority_op = self.main_wallet.deposit(amount_to_deposit).await?;
        self.monitor
            .wait_for_priority_op(BlockStatus::Committed, &priority_op)
            .await?;

        // Now when deposits are done it is time to update account id.
        self.main_wallet.update_account_id().await?;
        anyhow::ensure!(
            self.main_wallet.account_id().is_some(),
            "Account ID was not set after deposit for main account"
        );

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (tx, sign) = self
            .main_wallet
            .sign_change_pubkey(self.fees.zksync.clone())
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

            let txs = wait_all_failsafe_chunks(
                "executor/prepare/sign_transfer",
                CHUNK_SIZES,
                scenario_wallets.iter().map(|wallet| {
                    self.main_wallet.sign_transfer(
                        wallet.address(),
                        scenario_amount.clone(),
                        self.fees.zksync.clone(),
                    )
                }),
            )
            .await?;

            // Preserve transactions order to prevent "nonce mismatch" errors.
            let tx_hashes = wait_all_failsafe_chunks(
                "executor/prepare/wait_for_tx/send_tx",
                &[1],
                txs.into_iter()
                    .map(|(tx, sign)| self.monitor.send_tx(tx, sign)),
            )
            .await?;

            wait_all_failsafe_chunks(
                "executor/prepare/wait_for_tx/committed",
                CHUNK_SIZES,
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| self.monitor.wait_for_tx(BlockStatus::Committed, tx_hash)),
            )
            .await?;

            log::info!(
                "All the initial transfers for the `{}` scenario have been committed.",
                self.scenarios[scenario_index].0,
            );

            let tx_hashes = wait_all_failsafe_chunks(
                "executor/prepare/sign_change_pubkey",
                CHUNK_SIZES,
                scenario_wallets.iter_mut().map(|wallet| {
                    let fees = self.fees.clone();
                    let monitor = self.monitor.clone();
                    async move {
                        wallet.update_account_id().await?;

                        anyhow::ensure!(
                            wallet.account_id().is_some(),
                            "Account ID was not set after deposit for the account {}",
                            wallet.address().to_string()
                        );

                        let (tx, sign) = wallet.sign_change_pubkey(fees.zksync.clone()).await?;
                        monitor.send_tx(tx, sign).await
                    }
                }),
            )
            .await?;

            wait_all_failsafe_chunks(
                "executor/prepare/wait_for_tx/committed",
                CHUNK_SIZES,
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| self.monitor.wait_for_tx(BlockStatus::Committed, tx_hash)),
            )
            .await?;

            let scenario_handle = &mut self.scenarios[scenario_index];
            scenario_handle
                .0
                .prepare(&self.monitor, &self.fees, &scenario_wallets)
                .await?;
            scenario_handle.1 = scenario_wallets;

            log::info!(
                "All the preparation steps for the `{}` scenario have been finished.",
                self.scenarios[scenario_index].0,
            );
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
        let fees = self.fees.clone();
        wait_all_failsafe(
            "executor/process",
            self.scenarios
                .iter_mut()
                .map(|(scenario, wallets)| scenario.run(&monitor, &fees, wallets)),
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

        self.main_wallet.refresh_nonce().await?;
        // Transfer the remaining balances of the intermediate wallets into the main one.
        for (scenario, scenario_wallets) in &mut self.scenarios {
            let monitor = self.monitor.clone();
            let fees = self.fees.clone();
            scenario
                .finalize(&monitor, &fees, &scenario_wallets)
                .await?;

            let main_address = self.main_wallet.address();
            let txs_queue = wait_all_failsafe_chunks(
                "executor/refund/sign_transfer",
                CHUNK_SIZES,
                scenario_wallets.iter().map(|wallet| {
                    let zksync_fee = fees.zksync.clone();
                    async move {
                        wallet.refresh_nonce().await?;
                        let balance = wallet.balance(BlockStatus::Committed).await?;
                        // Make transfer back only if wallet has enough balance.
                        if balance > zksync_fee {
                            let withdraw_amount =
                                closest_packable_token_amount(&(balance - &zksync_fee));
                            let tx = wallet
                                .sign_transfer(
                                    main_address,
                                    withdraw_amount.clone(),
                                    zksync_fee.clone(),
                                )
                                .await?;

                            Ok(Some(tx)) as anyhow::Result<_>
                        } else {
                            Ok(None)
                        }
                    }
                }),
            )
            .await?;

            let tx_hashes = wait_all_failsafe_chunks(
                "executor/refund/send_tx",
                CHUNK_SIZES,
                txs_queue
                    .into_iter()
                    .filter_map(|x| x)
                    .map(|(tx, sign)| monitor.send_tx(tx, sign)),
            )
            .await?;

            wait_all_failsafe_chunks(
                "executor/refund/wait_for_tx/committed",
                CHUNK_SIZES,
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| monitor.wait_for_tx(BlockStatus::Committed, tx_hash)),
            )
            .await?;
        }

        // Withdraw remaining balance from the zkSync network back to the Ethereum one.
        let main_wallet_balance = self.main_wallet.balance(BlockStatus::Committed).await?;
        if main_wallet_balance > self.fees.zksync {
            log::info!(
                "Main wallet has {} balance, making refund...",
                format_ether(&main_wallet_balance)
            );

            let withdraw_amount =
                closest_packable_token_amount(&(main_wallet_balance - &self.fees.zksync));
            let (tx, sign) = self
                .main_wallet
                .sign_withdraw(withdraw_amount, self.fees.zksync.clone())
                .await?;
            self.monitor.send_tx(tx, sign).await?;
        }

        self.monitor.wait_for_verify().await;

        Ok(())
    }
}
