use futures::prelude::*;
use num::BigUint;
use structopt::StructOpt;

use models::{
    helpers::{closest_packable_fee_amount, closest_packable_token_amount},
    tx::PackedEthSignature,
    FranklinTx,
};
use zksync::types::BlockStatus;
use zksync_config::ConfigurationOptions;
use zksync_utils::format_ether;

use crate::{
    monitor::Monitor,
    scenarios::{configs::AccountInfo, utils::DynamicChunks},
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

async fn wait_all<I>(i: I) -> Vec<<I::Item as Future>::Output>
where
    I: IntoIterator,
    I::Item: Future,
{
    let mut output = Vec::new();
    for chunk in DynamicChunks::new(i, &[64]) {
        let values = futures::future::join_all(chunk).await;
        output.extend(values);
    }
    output
}

async fn try_wait_all<I>(
    i: I,
) -> Result<Vec<<I::Item as TryFuture>::Ok>, <I::Item as TryFuture>::Error>
where
    I: IntoIterator,
    I::Item: TryFuture,
{
    let mut output = Vec::new();
    for chunk in DynamicChunks::new(i, &[64]) {
        output.extend(futures::future::try_join_all(chunk).await?);
    }
    Ok(output)
}
