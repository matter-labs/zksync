use futures::prelude::*;
use num::BigUint;
use structopt::StructOpt;

use models::helpers::{closest_packable_fee_amount, closest_packable_token_amount};
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
    //// Transfer amount per accounts (in gwei).
    #[structopt(short = "t", default_value = "5")]
    pub transfer_size: u64,
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
        let wallets =
            wait_all((0..self.wallets).map(|_| TestWallet::new_random(monitor.clone(), &options)))
                .await;

        log::info!("Intermediate wallets are created");

        // Compute sufficient fee amount.
        let sufficient_fee = main_wallet.sufficient_fee().await?;
        let transfer_size = closest_packable_token_amount(&gwei_to_wei(self.transfer_size));

        // Make initial deposit.
        let amount_to_deposit: BigUint =
            (&transfer_size + &sufficient_fee) * BigUint::from(self.wallets);
        main_wallet
            .deposit(amount_to_deposit.clone())
            .await
            .unwrap();
        monitor.wait_for_verify().await;

        // Now when deposits are done it is time to update account id.
        main_wallet.update_account_id().await?;
        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        monitor
            .send_tx(
                main_wallet
                    .sign_change_pubkey(sufficient_fee.clone())
                    .await?,
                None,
            )
            .await?;
        // monitor.wait_for_verify().await;

        log::info!("Deposit phase completed");
        tokio::spawn(monitor.run_counter());

        // Split the money from the main account between the intermediate wallets.
        log::info!(
            "Preparing transactions for the initial transfer. {} ETH will be send to each of {} new wallets",
            format_ether(&transfer_size),
            self.wallets
        );

        let transfer_amount = &transfer_size + &sufficient_fee;
        let txs_queue = try_wait_all(wallets.iter().map(|to| {
            main_wallet.sign_transfer(
                to.address(),
                closest_packable_fee_amount(&transfer_amount),
                Some(sufficient_fee.clone()),
            )
        }))
        .await?;

        log::info!("All the initial transfers are created");
        try_wait_all(
            txs_queue
                .into_iter()
                .map(|(tx, sign)| monitor.send_tx(tx, sign)),
        )
        .await?;

        log::info!("Collected logs: {:#?}", monitor.take_logs().await);

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
