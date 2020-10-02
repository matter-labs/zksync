use futures::prelude::*;
use num::BigUint;
use structopt::StructOpt;

use models::{helpers::closest_packable_fee_amount};
use zksync_config::ConfigurationOptions;
use zksync_utils::format_ether;

use crate::{monitor::Monitor, scenarios::configs::AccountInfo, test_accounts::TestWallet};

#[derive(Debug, StructOpt)]
pub struct SimpleScenario {
    /// Number of intermediate wallets to use.
    #[structopt(short = "w", default_value = "100")]
    pub wallets: u64,
    //// Transfer amount per accounts (in wei).
    #[structopt(short = "t", default_value = "100")]
    pub transfer_size: BigUint,
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
        let wallets = futures::future::join_all(
            (0..self.wallets).map(|_| TestWallet::new_random(monitor.clone(), &options)),
        )
        .await;

        log::info!("Intermediate wallets are created");

        // Compute sufficient fee amount.
        let sufficient_fee = main_wallet
            .sufficient_fee()
            .await?;

        dbg!(&sufficient_fee);

        // Make initial deposit.
        let amount_to_deposit: BigUint =
            (&self.transfer_size - &sufficient_fee) * BigUint::from(self.wallets);
        main_wallet.deposit(amount_to_deposit.clone()).await?;
        // Now when deposits are done it is time to update account id.
        main_wallet.update_account_id().await?;
        // ...and change the main account pubkey.
        // We have to change pubkey after the deposit so we'll be able to use corresponding
        // `zkSync` account.
        monitor
            .send_tx(main_wallet.sign_change_pubkey(sufficient_fee.clone()).await?, None)
            .await?;

        // Wait for all the transactions to get verified.
        monitor.wait_for_verify().await;
        log::info!("Deposit phase completed");

        // Split the money from the main account between the intermediate wallets.
        log::info!(
            "Preparing transactions for the initial transfer. {} ETH will be send to each of {} new wallets",
            format_ether(&self.transfer_size),
            self.wallets
        );

        let transfer_amount = &self.transfer_size + &sufficient_fee;
        let txs_queue = futures::future::try_join_all(wallets.iter().map(|to| {
            main_wallet.sign_transfer(
                to.address(),
                closest_packable_fee_amount(&transfer_amount),
                Some(sufficient_fee.clone()),
            )
        }))
        .await?;

        log::info!("All the initial transfers are completed");
        futures::future::try_join_all(
            txs_queue
                .into_iter()
                .map(|(tx, sign)| monitor.send_tx(tx, sign)),
        )
        .await?;

        log::info!("Collected logs: {:#?}", monitor.take_logs().await);

        Ok(())
    }
}
