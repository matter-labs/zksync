// Built-in uses
use std::{collections::BTreeMap, fmt::Debug};

// External uses
use num::BigUint;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync::{
    types::BlockStatus,
    utils::{closest_packable_fee_amount, closest_packable_token_amount},
    RpcProvider,
};
use zksync_utils::format_ether;

// Local uses
use crate::{
    api::{self, ApiTestsFuture, ApiTestsReport, CancellationToken},
    config::{Config, NetworkConfig},
    journal::Journal,
    monitor::Monitor,
    scenarios::{Fees, Scenario, ScenariosTestsReport},
    utils::{
        gwei_to_wei, wait_all_chunks, wait_all_failsafe, wait_all_failsafe_chunks, CHUNK_SIZES,
    },
    wallet::{MainWallet, ScenarioWallet},
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
    main_wallet: MainWallet,
    monitor: Monitor,
    web3_url: String,
    /// Estimated fee amount for any zkSync operation.
    fees: Fees,
    scenarios: Vec<(Box<dyn Scenario>, Vec<ScenarioWallet>)>,
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
    /// The approximate number of extra operations for each wallet.
    const OPERATIONS_PER_WALLET: u64 = 5;
    /// Fee token of the main wallet.
    const MAIN_WALLET_FEE_TOKEN: &'static str = "ETH";

    /// Creates a new executor instance.
    pub async fn new(config: Config, web3_url: String) -> anyhow::Result<Self> {
        let monitor = Monitor::new(RpcProvider::new(config.network.name)).await;

        vlog::info!("Creating scenarios...");

        let scenarios = config
            .scenarios
            .into_iter()
            .map(|cfg| (cfg.into_scenario(), Vec::new()))
            .collect();

        // Create main account to deposit money from and to return money back later.
        let main_wallet =
            MainWallet::from_info(monitor.clone(), &config.main_wallet, &web3_url).await;

        let default_fee = main_wallet
            .sufficient_fee(Self::MAIN_WALLET_FEE_TOKEN)
            .await?;
        let fees = Fees::from_config(&config.network, default_fee);

        vlog::info!("Eth fee is {}", format_ether(&fees.eth));
        vlog::info!("zkSync fee is {}", format_ether(&fees.zksync));

        let api_tests = api::run(monitor.clone());

        Ok(Self {
            monitor,
            web3_url,
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
        let total_fee = BigUint::from(Self::OPERATIONS_PER_WALLET) * &self.fees.zksync;

        let mut wallets = Vec::new();
        let mut deposit_ops = Vec::new();
        for resource in resources {
            let token_name = &resource.token_name;

            let wallet_balance = resource.balance_per_wallet + &total_fee;
            let scenario_amount =
                (&wallet_balance + &total_fee) * BigUint::from(resource.wallets_amount);

            let scenario_wallets = wait_all_chunks(
                CHUNK_SIZES,
                (0..resource.wallets_amount).map(|_| {
                    ScenarioWallet::new_random(
                        token_name.clone(),
                        self.monitor.clone(),
                        &self.web3_url,
                    )
                }),
            )
            .await;

            // Special case for erc20 tokens.
            if !token_name.is_eth() {
                self.main_wallet.approve_erc20_deposits(token_name).await?;
            }

            if !token_name.is_eth() && resource.has_deposits {
                vlog::info!(
                    "Approving {} wallets for {} deposits.",
                    scenario_wallets.len(),
                    token_name,
                );

                for wallet in &scenario_wallets {
                    // Give some gas to make it possible to create Ethereum transactions.
                    let eth_balance =
                        closest_packable_fee_amount(&(&self.fees.eth * BigUint::from(2_u64)));
                    self.main_wallet
                        .transfer_to("ETH", eth_balance, wallet.address())
                        .await?;
                    wallet.approve_erc20_deposits().await?;
                }

                vlog::info!(
                    "All of {} wallets have been approved for deposits.",
                    scenario_wallets.len(),
                );
            }

            // Make deposit from Ethereum network to the zkSync one.
            let amount_to_deposit = closest_packable_token_amount(
                &(&scenario_amount + BigUint::from(Self::OPERATIONS_PER_WALLET)),
            );

            let l1_balance = self.main_wallet.l1_balance(token_name).await?;
            anyhow::ensure!(
                l1_balance > amount_to_deposit,
                "Not enough balance in the main wallet to perform this test, actual: {}, expected: {}",
                format_ether(&l1_balance),
                format_ether(&amount_to_deposit),
            );

            vlog::info!(
                "Deposit {} {} for main wallet",
                format_ether(&amount_to_deposit),
                token_name,
            );

            let priority_op = self
                .main_wallet
                .deposit(token_name, amount_to_deposit)
                .await?;

            deposit_ops.push(priority_op);
            wallets.push((scenario_wallets, wallet_balance));
        }

        // Wait for pending priority operations
        for priority_op in deposit_ops {
            self.monitor
                .wait_for_priority_op(BlockStatus::Committed, &priority_op)
                .await?;
        }

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
            .sign_change_pubkey(Self::MAIN_WALLET_FEE_TOKEN, self.fees.zksync.clone())
            .await?;
        let tx_hash = self.monitor.send_tx(tx, sign).await?;
        self.monitor
            .wait_for_tx(BlockStatus::Committed, tx_hash)
            .await?;

        vlog::info!("Deposit phase completed");

        // Split the money from the main account between the intermediate wallets.
        for (scenario_index, (mut scenario_wallets, scenario_amount)) in
            wallets.into_iter().enumerate()
        {
            vlog::info!(
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
                    let amount = closest_packable_token_amount(&scenario_amount);
                    self.main_wallet.sign_transfer(
                        wallet.token_name(),
                        wallet.address(),
                        amount,
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

            vlog::info!(
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

            vlog::info!(
                "All the preparation steps for the `{}` scenario have been finished.",
                self.scenarios[scenario_index].0,
            );
        }

        vlog::info!("Awaiting for pending tasks verification...",);
        self.monitor.wait_for_verify().await;
        Ok(())
    }

    /// Performs main step of the load tests.
    async fn process(&mut self) -> anyhow::Result<Journal> {
        vlog::info!("Starting TPS measuring...");
        let monitor = self.monitor.clone();
        monitor.start().await;

        // Run scenarios concurrently.
        let fees = self.fees.clone();
        self.scenarios = wait_all_failsafe(
            "executor/process_par",
            self.scenarios
                .drain(..)
                .map(move |(mut scenario, wallets)| {
                    let monitor = monitor.clone();
                    let fees = fees.clone();
                    async move {
                        tokio::spawn(async move {
                            let wallets = scenario.run(monitor, fees, wallets).await?;
                            Ok((scenario, wallets)) as anyhow::Result<_>
                        })
                        .await?
                    }
                }),
        )
        .await?;
        self.monitor.wait_for_verify().await;

        let logs = self.monitor.finish().await;
        vlog::info!("TPS measuring finished...");
        Ok(logs)
    }

    /// Returns the remaining funds to the main wallet.
    async fn refund(&mut self) -> anyhow::Result<()> {
        vlog::info!("Refunding the remaining tokens to the main wallet.");

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

            for wallet in scenario_wallets {
                // Refund remaining erc20 tokens to the main wallet
                if !wallet.token_name().is_eth() {
                    let balance = wallet.erc20_balance().await?;
                    if balance > self.fees.eth {
                        let amount = balance - &self.fees.eth;
                        wallet
                            .transfer_to(
                                wallet.token_name().clone(),
                                closest_packable_token_amount(&amount),
                                main_address,
                            )
                            .await?;
                    }
                }
                // Move remaining gas to the main wallet.
                let balance = wallet.eth_balance().await?;
                if balance > self.fees.eth {
                    let amount = balance - &self.fees.eth;
                    wallet
                        .transfer_to("ETH", closest_packable_token_amount(&amount), main_address)
                        .await?;
                }
            }

            // Withdraw remaining balance from the zkSync network back to the Ethereum one.
            let token_name = scenario.requested_resources(&self.fees).token_name;

            let main_wallet_balance = self
                .main_wallet
                .balance(&token_name, BlockStatus::Committed)
                .await?;
            if main_wallet_balance > self.fees.zksync {
                vlog::info!(
                    "Main wallet has {} {} balance, making refund...",
                    format_ether(&main_wallet_balance),
                    token_name,
                );

                let withdraw_amount =
                    closest_packable_token_amount(&(main_wallet_balance - &self.fees.zksync));
                let (tx, sign) = self
                    .main_wallet
                    .sign_withdraw(token_name, withdraw_amount, self.fees.zksync.clone())
                    .await?;
                self.monitor.send_tx(tx, sign).await?;
            }
        }

        self.monitor.wait_for_verify().await;

        Ok(())
    }
}
