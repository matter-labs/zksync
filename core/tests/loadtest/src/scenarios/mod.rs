// Built-in uses
use std::fmt::{Debug, Display};
// External uses
use async_trait::async_trait;
use num::BigUint;
// Workspace uses
use zksync::{types::BlockStatus, utils::closest_packable_token_amount};
use zksync_config::ConfigurationOptions;
use zksync_utils::format_ether;
// Local uses
use crate::{
    config::AccountInfo,
    journal::Journal,
    monitor::Monitor,
    test_wallet::TestWallet,
    utils::{try_wait_all, wait_all},
};

mod transfers;

/// Resources that are needed from the scenario executor to perform the scenario.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ScenarioResources {
    /// Total amount of non-empty wallets.
    pub wallets_amount: u64,
    /// Wei balance in each wallet.
    pub balance_per_wallet: BigUint,
}

#[async_trait]
pub trait Scenario: Debug + Display {
    /// Returns resources that should be provided by the scenario executor.
    fn requested_resources(&self, sufficient_fee: &BigUint) -> ScenarioResources;

    async fn prepare(
        &mut self,
        monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallets: &[TestWallet],
    ) -> anyhow::Result<()>;

    async fn run(
        &mut self,
        monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallets: &[TestWallet],
    ) -> anyhow::Result<()>;

    async fn finalize(
        &mut self,
        monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallets: &[TestWallet],
    ) -> anyhow::Result<()>;
}

#[derive(Debug)]
pub struct ScenarioExecutor {
    /// Main account to deposit ETH from / return ETH back to.
    main_wallet: TestWallet,
    monitor: Monitor,
    options: ConfigurationOptions,
    /// Estimated fee amount for any zkSync operation.
    sufficient_fee: BigUint,
    scenarios: Vec<(Box<dyn Scenario>, Vec<TestWallet>)>,
}

impl ScenarioExecutor {
    pub async fn new(
        monitor: Monitor,
        main_account: AccountInfo,
        options: ConfigurationOptions,
    ) -> anyhow::Result<Self> {
        log::info!("Creating scenarious...");

        let scenario = Box::new(transfers::TransferScenario::default());

        // Create main account to deposit money from and to return money back later.
        let main_wallet = TestWallet::from_info(monitor.clone(), &main_account, &options).await;
        let sufficient_fee = main_wallet.sufficient_fee().await?;

        Ok(Self {
            monitor,
            options,
            main_wallet,
            scenarios: vec![(scenario, Vec::new())],
            sufficient_fee,
        })
    }

    pub async fn run(mut self) -> anyhow::Result<Journal> {
        self.prepare().await?;
        let logs = self.process().await?;
        self.refund().await?;

        Ok(logs)
    }

    /// Makes initial deposit to the main wallet.
    async fn prepare(&mut self) -> anyhow::Result<()> {
        // Create requested wallets and make initial deposit with the sufficient amount.
        let resources = self
            .scenarios
            .iter()
            .map(|x| x.0.requested_resources(&self.sufficient_fee));

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

            let scenario_wallets = wait_all(
                (0..resource.wallets_amount)
                    .map(|_| TestWallet::new_random(self.monitor.clone(), &self.options)),
            )
            .await;

            wallets.push((scenario_wallets, wallet_balance));
        }

        let amount_to_deposit = closest_packable_token_amount(&amount_to_deposit);

        let eth_balance = self.main_wallet.eth_balance().await?;
        anyhow::ensure!(
            eth_balance > amount_to_deposit,
            "Not enough balance in the main wallet to perform this test, actual: {} ETH, expected: {} ETH",
            format_ether(&eth_balance),
            format_ether(&amount_to_deposit),
        );

        log::info!(
            "Deposit {} ETH for main wallet",
            format_ether(&amount_to_deposit),
        );

        let priority_op = self.main_wallet.deposit(amount_to_deposit).await.unwrap();
        self.monitor
            .wait_for_priority_op_commit(&priority_op)
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
        self.monitor.wait_for_tx_commit(tx_hash).await?;

        log::info!("Deposit phase completed");

        // Split the money from the main account between the intermediate wallets.
        for (scenario_index, (mut scenario_wallets, scenario_amount)) in
            wallets.into_iter().enumerate()
        {
            log::info!(
                "Preparing transactions for the initial transfer for `{}` scenario: \
                {} ETH to will be send to each of {} new wallets",
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

            try_wait_all(
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| self.monitor.wait_for_tx_commit(tx_hash)),
            )
            .await?;

            log::info!("All the initial transfers have been committed.");

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

            try_wait_all(
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| self.monitor.wait_for_tx_commit(tx_hash)),
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

    async fn process(&mut self) -> anyhow::Result<Journal> {
        log::info!("Starting TPS measuring...");
        let monitor = self.monitor.clone();
        monitor.start().await;

        let sufficient_fee = self.sufficient_fee.clone();
        try_wait_all(
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

    async fn refund(&mut self) -> anyhow::Result<()> {
        log::info!("Refunding the remaining tokens to the main wallet.");

        for (scenario, scenario_wallets) in &mut self.scenarios {
            let monitor = self.monitor.clone();
            let sufficient_fee = self.sufficient_fee.clone();
            scenario
                .finalize(&monitor, &sufficient_fee, &scenario_wallets)
                .await?;

            let main_address = self.main_wallet.address();
            let txs_queue = try_wait_all(scenario_wallets.iter().map(|wallet| {
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

            let tx_hashes = try_wait_all(
                txs_queue
                    .into_iter()
                    .map(|(tx, sign)| monitor.send_tx(tx, sign)),
            )
            .await?;

            try_wait_all(
                tx_hashes
                    .into_iter()
                    .map(|tx_hash| monitor.wait_for_tx_commit(tx_hash)),
            )
            .await?;
        }

        let main_wallet_balance = self.main_wallet.balance(BlockStatus::Committed).await?;
        if main_wallet_balance > self.sufficient_fee {
            log::info!(
                "Main wallet has {} ETH balance, making refund...",
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
