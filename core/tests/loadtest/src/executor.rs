// Built-in uses
use std::{collections::BTreeMap, fmt::Debug};

// External uses
use futures::future::try_join_all;
use num::BigUint;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync::{
    types::BlockStatus,
    utils::{closest_packable_fee_amount, closest_packable_token_amount},
    Network, RpcProvider,
};
use zksync_types::TokenLike;
use zksync_utils::format_ether;

// Local uses
use crate::{
    api::{self, ApiTestsFuture, ApiTestsReport, CancellationToken},
    config::Config,
    journal::Journal,
    monitor::Monitor,
    scenarios::{Fees, Scenario, ScenarioConfig, ScenariosTestsReport},
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

struct ScenarioData {
    inner: Box<dyn Scenario>,
    wallets: Vec<ScenarioWallet>,
    fees: Fees,
}

impl ScenarioData {
    async fn new(
        main_wallet: &MainWallet,
        cfg: ScenarioConfig,
        eth_fee: BigUint,
    ) -> anyhow::Result<Self> {
        let zksync_fee = if let Some(fee) = cfg.zksync_fee {
            gwei_to_wei(fee)
        } else {
            main_wallet.sufficient_fee(&cfg.token_name).await?
        };

        Ok(Self {
            inner: cfg.into_scenario(),
            fees: Fees {
                eth: closest_packable_fee_amount(&eth_fee),
                zksync: closest_packable_fee_amount(&zksync_fee),
            },
            wallets: Vec::new(),
        })
    }
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
    network: Network,
    /// Estimated fee amount for any zkSync operation.
    fees: Fees,
    fee_token: TokenLike,
    scenarios: Vec<ScenarioData>,
    api_tests: Option<(ApiTestsFuture, CancellationToken)>,
}

fn fee_from_config_field(config_field: &Option<u64>, default_fee: BigUint) -> BigUint {
    closest_packable_fee_amount(&config_field.map(gwei_to_wei).unwrap_or(default_fee))
}

impl LoadtestExecutor {
    /// The approximate number of extra operations for each wallet.
    const OPERATIONS_PER_WALLET: u64 = 5;

    /// Creates a new executor instance.
    pub async fn new(config: Config, web3_url: String) -> anyhow::Result<Self> {
        let monitor = Monitor::new(RpcProvider::new(config.network.name)).await;

        vlog::info!("Creating scenarios...");

        // Create main account to deposit money from and to return money back later.
        let main_wallet = MainWallet::new(
            monitor.clone(),
            config.network.name,
            config.main_wallet.credentials,
            &web3_url,
        )
        .await;

        let default_fee = main_wallet
            .sufficient_fee(&config.main_wallet.fee_token)
            .await?;

        let fees = Fees {
            eth: closest_packable_fee_amount(&gwei_to_wei(config.network.eth_fee)),
            zksync: fee_from_config_field(&config.main_wallet.zksync_fee, default_fee),
        };

        let scenarios = try_join_all(
            config
                .scenarios
                .into_iter()
                .map(|cfg| ScenarioData::new(&main_wallet, cfg, fees.eth.clone())),
        )
        .await?;

        vlog::info!("Eth fee is {}", format_ether(&fees.eth));
        vlog::info!("Main wallet zkSync fee is {}", format_ether(&fees.zksync));

        let api_tests = api::run(monitor.clone());

        Ok(Self {
            monitor,
            web3_url,
            main_wallet,
            scenarios,
            fee_token: config.main_wallet.fee_token,
            network: config.network.name,
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
        let resources = self.scenarios.iter().map(|x| {
            let resource = x.inner.requested_resources(&x.fees);
            (resource, x.fees.clone())
        });

        // Create intermediate wallets and compute total amount to deposit and needed
        // balances for wallets.

        let mut wallets = Vec::new();
        let mut deposit_ops = Vec::new();
        for (resource, fees) in resources {
            let token_name = &resource.token_name;
            let total_fee = BigUint::from(Self::OPERATIONS_PER_WALLET) * (&fees.zksync + &fees.eth);

            vlog::info!(
                "For token {} zkSync fee is {}",
                token_name,
                format_ether(&fees.zksync)
            );

            let wallet_balance = resource.balance_per_wallet + &total_fee;
            let scenario_amount =
                (&wallet_balance + &total_fee) * BigUint::from(resource.wallets_amount);

            let scenario_wallets = wait_all_chunks(
                CHUNK_SIZES,
                (0..resource.wallets_amount).map(|_| {
                    ScenarioWallet::new_random(
                        self.monitor.clone(),
                        self.network,
                        token_name.clone(),
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
                    let eth_balance = closest_packable_fee_amount(
                        &(&fees.eth * BigUint::from(Self::OPERATIONS_PER_WALLET)),
                    );
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
                &(&scenario_amount + BigUint::from(Self::OPERATIONS_PER_WALLET) * &fees.zksync),
            );

            let l1_balance = self.main_wallet.l1_balance(token_name).await?;
            anyhow::ensure!(
                l1_balance > amount_to_deposit,
                "Not enough balance in the main wallet to perform this test, actual: {}, expected: {}",
                format_ether(&l1_balance),
                format_ether(&amount_to_deposit),
            );

            vlog::info!(
                "Deposit {} {} for scenario",
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

        // Deposit some ETH to the L2 network for the main wallet to process `ChangePubKey`
        // transaction
        let main_wallet_amount = &self.fees.zksync * BigUint::from(Self::OPERATIONS_PER_WALLET);

        vlog::info!(
            "Deposit {} {} for main wallet",
            format_ether(&main_wallet_amount),
            self.fee_token,
        );

        let priority_op = self
            .main_wallet
            .deposit(&self.fee_token, main_wallet_amount)
            .await?;
        deposit_ops.push(priority_op);

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
            .sign_change_pubkey(&self.fee_token, self.fees.zksync.clone())
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
                self.scenarios[scenario_index].inner,
                format_ether(&scenario_amount),
                scenario_wallets.len()
            );

            let txs = wait_all_failsafe_chunks(
                "executor/prepare/sign_transfer",
                CHUNK_SIZES,
                scenario_wallets.iter().map(|wallet| {
                    let amount = closest_packable_token_amount(&scenario_amount);
                    let fee = self.scenarios[scenario_index].fees.zksync.clone();

                    self.main_wallet.sign_transfer(
                        wallet.token_name(),
                        wallet.address(),
                        amount,
                        fee,
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
                self.scenarios[scenario_index].inner,
            );

            let tx_hashes = wait_all_failsafe_chunks(
                "executor/prepare/sign_change_pubkey",
                CHUNK_SIZES,
                scenario_wallets.iter_mut().map(|wallet| {
                    let fee = self.scenarios[scenario_index].fees.zksync.clone();
                    let monitor = self.monitor.clone();

                    async move {
                        wallet.update_account_id().await?;

                        anyhow::ensure!(
                            wallet.account_id().is_some(),
                            "Account ID was not set after deposit for the account {}",
                            wallet.address().to_string()
                        );

                        let (tx, sign) = wallet.sign_change_pubkey(fee).await?;
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
                .inner
                .prepare(&self.monitor, &scenario_handle.fees, &scenario_wallets)
                .await?;
            scenario_handle.wallets = scenario_wallets;

            vlog::info!(
                "All the preparation steps for the `{}` scenario have been finished.",
                self.scenarios[scenario_index].inner,
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
        self.scenarios = wait_all_failsafe(
            "executor/process_par",
            self.scenarios.drain(..).map(move |mut scenario| {
                let monitor = monitor.clone();
                async move {
                    tokio::spawn(async move {
                        scenario.wallets = scenario
                            .inner
                            .run(monitor, scenario.fees.clone(), scenario.wallets)
                            .await?;
                        Ok(scenario) as anyhow::Result<_>
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
        for scenario in &mut self.scenarios {
            let monitor = self.monitor.clone();
            scenario
                .inner
                .finalize(&monitor, &scenario.fees, &scenario.wallets)
                .await?;

            let main_address = self.main_wallet.address();
            let txs_queue = wait_all_failsafe_chunks(
                "executor/refund/sign_transfer",
                CHUNK_SIZES,
                scenario.wallets.iter().map(|wallet| {
                    let zksync_fee = scenario.fees.zksync.clone();
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
                    .flatten()
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

            for wallet in &scenario.wallets {
                // Refund remaining erc20 tokens to the main wallet
                if !wallet.token_name().is_eth() {
                    let balance = wallet.erc20_balance().await?;
                    if balance > scenario.fees.eth {
                        let amount = balance - &scenario.fees.eth;
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
                if balance > scenario.fees.eth {
                    let amount = balance - &scenario.fees.eth;
                    wallet
                        .transfer_to("ETH", closest_packable_token_amount(&amount), main_address)
                        .await?;
                }
            }

            // Withdraw remaining balance from the zkSync network back to the Ethereum one.
            let token_name = scenario
                .inner
                .requested_resources(&scenario.fees)
                .token_name;

            let main_wallet_balance = self
                .main_wallet
                .balance(&token_name, BlockStatus::Committed)
                .await?;
            if main_wallet_balance > scenario.fees.zksync {
                vlog::info!(
                    "Main wallet has {} {} balance, making refund...",
                    format_ether(&main_wallet_balance),
                    token_name,
                );

                let withdraw_amount =
                    closest_packable_token_amount(&(main_wallet_balance - &scenario.fees.zksync));
                let (tx, sign) = self
                    .main_wallet
                    .sign_withdraw(token_name, withdraw_amount, scenario.fees.zksync.clone())
                    .await?;
                self.monitor.send_tx(tx, sign).await?;
            }
        }

        self.monitor.wait_for_verify().await;

        Ok(())
    }
}
