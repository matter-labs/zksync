// Built-in uses
// External uses
use async_trait::async_trait;
use num::BigUint;
use structopt::StructOpt;
// Workspace uses
use zksync::{types::BlockStatus, utils::closest_packable_token_amount};
use zksync_config::ConfigurationOptions;
use zksync_types::{tx::PackedEthSignature, ZkSyncTx};
use zksync_utils::format_ether;
// Local uses
use super::{Scenario, ScenarioResources};
use crate::{
    monitor::Monitor,
    ng::utils::{try_wait_all, wait_all},
    scenarios::configs::AccountInfo,
    test_accounts::TestWallet,
};

#[derive(Debug, StructOpt)]
pub struct SimpleScenario {
    /// Number of intermediate wallets to use.
    #[structopt(short = "w", default_value = "100")]
    pub wallets: u64,
    /// Transfer amount per accounts (in gwei).
    #[structopt(short = "t", default_value = "100")]
    pub transfer_size: u64,
    /// Number of transfer rounds.
    #[structopt(short = "n", default_value = "1")]
    pub transfer_rounds: u64,
}

fn gwei_to_wei(gwei: impl Into<BigUint>) -> BigUint {
    gwei.into() * BigUint::from(10u64.pow(9))
}

impl SimpleScenario {
    pub async fn run(
        self,
        monitor: Monitor,
        main_account: AccountInfo,
        options: ConfigurationOptions,
    ) -> Result<(), anyhow::Error> {
        log::info!("Starting simple scenario");

        // Create main account to deposit money from and to return money back later.
        let mut main_wallet = TestWallet::from_info(monitor.clone(), &main_account, &options).await;
        // Generate random accounts to rotate funds within.
        let mut wallets =
            wait_all((0..self.wallets).map(|_| TestWallet::new_random(monitor.clone(), &options)))
                .await;

        log::info!("Intermediate wallets are created");

        // Compute sufficient fee amount.
        let sufficient_fee = main_wallet.sufficient_fee().await?;
        let transfer_size = gwei_to_wei(self.transfer_size);

        // Make initial deposit.

        // TODO Use minimal sufficient amount.
        let amount_to_deposit = (&transfer_size + &sufficient_fee)
            * BigUint::from(self.wallets * self.transfer_rounds * 5);
        let amount_to_deposit = closest_packable_token_amount(&amount_to_deposit);

        let eth_balance = main_wallet.eth_provider.balance().await?;
        anyhow::ensure!(
            eth_balance > amount_to_deposit,
            "Not enough balance in the main wallet to perform this test, actual: {}, expected: {}",
            format_ether(&eth_balance),
            format_ether(&amount_to_deposit),
        );

        log::info!(
            "Deposit {} for main wallet, sufficient fee is {}",
            format_ether(&amount_to_deposit),
            format_ether(&sufficient_fee),
        );
        let priority_op = main_wallet.deposit(amount_to_deposit).await.unwrap();
        monitor.wait_for_priority_op_commit(&priority_op).await?;

        // Now when deposits are done it is time to update account id.
        main_wallet.update_account_id().await?;
        assert!(
            main_wallet.account_id().is_some(),
            "Account ID was not set after deposit for main account"
        );

        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        let (tx, sign) = main_wallet
            .sign_change_pubkey(sufficient_fee.clone())
            .await?;
        let tx_hash = monitor.send_tx(tx, sign).await?;
        monitor.wait_for_tx_commit(tx_hash).await?;

        log::info!("Deposit phase completed");

        // Split the money from the main account between the intermediate wallets.
        log::info!(
            "Preparing transactions for the initial transfer. {} ETH will be send to each of {} new wallets",
            format_ether(&transfer_size),
            self.wallets
        );

        let transfer_amount =
            &transfer_size + (&sufficient_fee * BigUint::from(self.transfer_rounds + 2));
        let transfer_amount = closest_packable_token_amount(&transfer_amount);

        // TODO Replace copy-paste by the generic solution.

        let mut tx_hashes = Vec::new();
        for wallet in &wallets {
            let (tx, sign) = main_wallet
                .sign_transfer(
                    wallet.address(),
                    transfer_amount.clone(),
                    Some(sufficient_fee.clone()),
                )
                .await?;
            tx_hashes.push(monitor.send_tx(tx, sign).await?);
        }

        try_wait_all(
            tx_hashes
                .into_iter()
                .map(|tx_hash| monitor.wait_for_tx_commit(tx_hash)),
        )
        .await?;

        log::info!("All the initial transfers have been committed.");

        let mut tx_hashes = Vec::new();
        for wallet in &mut wallets {
            wallet.update_account_id().await?;
            assert!(
                wallet.account_id().is_some(),
                "Account ID was not set after deposit for the account"
            );

            let (tx, sign) = wallet.sign_change_pubkey(sufficient_fee.clone()).await?;
            tx_hashes.push(monitor.send_tx(tx, sign).await?);
        }

        try_wait_all(
            tx_hashes
                .into_iter()
                .map(|tx_hash| monitor.wait_for_tx_commit(tx_hash)),
        )
        .await?;

        // Run transfers step.
        let transfers_number = (self.wallets * self.transfer_rounds) as usize;
        log::info!(
            "All the initial transfers have been verified, creating {} transactions \
            for the transfers step",
            transfers_number
        );
        let txs_queue = try_wait_all((0..transfers_number).map(|i| {
            let from = i % wallets.len();
            let to = (i + 1) % wallets.len();

            wallets[from].sign_transfer(
                wallets[to].address(),
                closest_packable_token_amount(&transfer_size),
                Some(sufficient_fee.clone()),
            )
        }))
        .await?;

        log::info!(
            "Created {} transactions, awaiting for pending tasks verification...",
            txs_queue.len()
        );
        monitor.wait_for_verify().await;
        log::info!("Starting TPS measuring...");

        tokio::spawn(monitor.run_counter());
        try_wait_all(
            txs_queue
                .into_iter()
                .map(|(tx, sign)| monitor.send_tx(tx, sign)),
        )
        .await?;
        monitor.wait_for_verify().await;
        let logs = monitor.take_logs().await;

        // Refunding stage.
        log::info!("Refunding the remaining tokens to the main wallet.");
        let txs_queue = try_wait_all(wallets.into_iter().map(|wallet| {
            let sufficient_fee = sufficient_fee.clone();
            let main_address = main_wallet.address();

            async move {
                let balance = wallet.balance(BlockStatus::Verified).await?;
                let withdraw_amount = closest_packable_token_amount(&(balance - &sufficient_fee));

                wallet
                    .sign_transfer(
                        main_address,
                        withdraw_amount.clone(),
                        Some(sufficient_fee.clone()),
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

        let main_wallet_balance = main_wallet.balance(BlockStatus::Committed).await?;
        if main_wallet_balance > sufficient_fee {
            log::info!(
                "Main wallet has {} balance, making refund...",
                format_ether(&main_wallet_balance)
            );

            let withdraw_amount =
                closest_packable_token_amount(&(main_wallet_balance - &sufficient_fee));
            let (tx, sign) = main_wallet
                .sign_withdraw(withdraw_amount, Some(sufficient_fee))
                .await?;
            monitor.send_tx(tx, sign).await?;
        }
        monitor.wait_for_verify().await;

        log::trace!("Collected logs: {:#?}", logs);

        Ok(())
    }
}

#[derive(Debug)]
pub struct ScenarioExecutor {
    monitor: Monitor,
    options: ConfigurationOptions,
    main_wallet: TestWallet,
    sufficient_fee: BigUint,
    // TODO Convert to vector.
    scenarios: Vec<(Box<dyn Scenario>, Vec<TestWallet>)>,
}

impl ScenarioExecutor {
    pub async fn new(
        monitor: Monitor,
        main_account: AccountInfo,
        options: ConfigurationOptions,
    ) -> anyhow::Result<Self> {
        log::info!("Creating scenarious...");

        let scenario = Box::new(SimpleScenarioImpl::default());

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

    pub async fn run(mut self) -> anyhow::Result<()> {
        self.prepare().await?;
        self.process().await?;
        self.refund().await?;

        Ok(())
    }

    /// Makes initial deposit to the main wallet.
    async fn prepare(&mut self) -> anyhow::Result<()> {
        let resources = self
            .scenarios
            .iter()
            .map(|x| x.0.requested_resources(&self.sufficient_fee));

        dbg!(format_ether(&self.sufficient_fee));

        let mut amount_to_deposit = BigUint::from(0_u64);
        let mut wallets = Vec::new();
        for resource in resources {
            let wallet_balance = closest_packable_token_amount(
                &(&resource.balance_per_wallet + BigUint::from(2_u64) * &self.sufficient_fee),
            );
            let scenario_amount = closest_packable_token_amount(
                &(resource.wallets_amount * &wallet_balance + &self.sufficient_fee),
            );

            dbg!(format_ether(&wallet_balance));
            dbg!(format_ether(&scenario_amount));

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

        let eth_balance = self.main_wallet.eth_provider.balance().await?;
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
                "Preparing transactions for the initial transfer for. {} \
                ETH will be send to each of {} new wallets",
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
                        Some(self.sufficient_fee.clone()),
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

    async fn process(&mut self) -> anyhow::Result<()> {
        log::info!("Starting TPS measuring...");

        tokio::spawn(self.monitor.run_counter());

        let monitor = self.monitor.clone();
        let sufficient_fee = self.sufficient_fee.clone();
        try_wait_all(
            self.scenarios
                .iter_mut()
                .map(|(scenario, wallets)| scenario.run(&monitor, &sufficient_fee, wallets)),
        )
        .await?;
        self.monitor.wait_for_verify().await;
        let logs = self.monitor.take_logs().await;

        log::info!("TPS measuring finished...");

        log::trace!("Collected logs: {:#?}", logs);

        Ok(())
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
                            Some(sufficient_fee.clone()),
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
                "Main wallet has {} balance, making refund...",
                format_ether(&main_wallet_balance)
            );

            let withdraw_amount =
                closest_packable_token_amount(&(main_wallet_balance - &self.sufficient_fee));
            let (tx, sign) = self
                .main_wallet
                .sign_withdraw(withdraw_amount, Some(self.sufficient_fee.clone()))
                .await?;
            self.monitor.send_tx(tx, sign).await?;
        }

        self.monitor.wait_for_verify().await;

        Ok(())
    }
}

#[derive(Debug)]
pub struct SimpleScenarioImpl {
    transfer_size: BigUint,
    transfer_rounds: u64,
    wallets: u64,
    txs: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
}

impl Default for SimpleScenarioImpl {
    fn default() -> Self {
        Self {
            transfer_size: BigUint::from(1_000_000_u64),
            transfer_rounds: 10,
            wallets: 100,
            txs: Vec::new(),
        }
    }
}

#[async_trait]
impl Scenario for SimpleScenarioImpl {
    fn name(&self) -> &str {
        "simple"
    }

    fn requested_resources(&self, fee: &BigUint) -> ScenarioResources {
        let balance_per_wallet = &self.transfer_size + (fee * BigUint::from(self.transfer_rounds));

        ScenarioResources {
            balance_per_wallet: closest_packable_token_amount(&balance_per_wallet),
            wallets_amount: self.wallets,
        }
    }

    async fn prepare(
        &mut self,
        _monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        let transfers_number = (self.wallets * self.transfer_rounds) as usize;

        log::info!(
            "Simple scenario: All the initial transfers have been verified, creating {} transactions \
            for the transfers step",
            transfers_number
        );

        self.txs = try_wait_all((0..transfers_number).map(|i| {
            let from = i % wallets.len();
            let to = (i + 1) % wallets.len();

            wallets[from].sign_transfer(
                wallets[to].address(),
                closest_packable_token_amount(&self.transfer_size),
                Some(sufficient_fee.clone()),
            )
        }))
        .await?;

        log::info!(
            "Simple scenario: created {} transactions...",
            self.txs.len()
        );

        Ok(())
    }

    async fn run(
        &mut self,
        monitor: &Monitor,
        _sufficient_fee: &BigUint,
        _wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        try_wait_all(
            self.txs
                .drain(..)
                .map(|(tx, sign)| monitor.send_tx(tx, sign)),
        )
        .await?;

        Ok(())
    }

    async fn finalize(
        &mut self,
        _monitor: &Monitor,
        _sufficient_fee: &BigUint,
        _wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}
