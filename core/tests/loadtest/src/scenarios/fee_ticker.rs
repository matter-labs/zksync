// Built-in uses
use std::fmt;

// External uses
use async_trait::async_trait;
use num::BigUint;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync::provider::Provider;
use zksync::utils::closest_packable_token_amount;
use zksync_types::{TokenLike, TxFeeTypes};
use zksync_utils::format_ether;

// Local uses
use super::{batch_transfers::batch_sizes_iter, Fees, Scenario, ScenarioResources};
use crate::{
    monitor::Monitor,
    utils::{gwei_to_wei, wait_all_failsafe, wait_all_failsafe_chunks, DynamicChunks, CHUNK_SIZES},
    wallet::ScenarioWallet,
};

/// Configuration options for the fee ticker scenario.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct FeeTickerScenarioConfig {
    /// Amount of money to be used in the transfer, in gwei.
    pub transfer_size: u64,
    /// Amount of iterations to rotate funds, "length" of the test.
    pub transfer_rounds: u64,
    /// Amount of intermediate wallets to use.
    ///
    /// Due to scenario implementation details, amount of intermediate wallets
    /// should be greater than the expected block size.
    pub wallets_amount: u64,
    /// Maximum transactions batch size.
    ///
    /// The test uses the following batch sizes: `[2, max_batch_size / 2, max_batch_size]`
    pub max_batch_size: Option<u64>,
}

impl Default for FeeTickerScenarioConfig {
    fn default() -> Self {
        Self {
            transfer_size: 1,
            transfer_rounds: 10,
            wallets_amount: 100,
            max_batch_size: None,
        }
    }
}

#[derive(Debug)]
pub struct FeeTickerScenario {
    token_name: TokenLike,
    transfer_size: BigUint,
    transfer_rounds: u64,
    wallets: usize,
    max_batch_size: Option<usize>,
}

impl fmt::Display for FeeTickerScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "fee_ticker({})", self.token_name)
    }
}

#[async_trait]
impl Scenario for FeeTickerScenario {
    fn requested_resources(&self, fees: &Fees) -> ScenarioResources {
        let balance_per_wallet =
            &self.transfer_size + (&fees.zksync * BigUint::from(self.transfer_rounds));

        ScenarioResources {
            balance_per_wallet: closest_packable_token_amount(&balance_per_wallet),
            wallets_amount: self.wallets as u64,
            token_name: self.token_name.clone(),
            has_deposits: false,
        }
    }

    async fn prepare(
        &mut self,
        _monitor: &Monitor,
        _fees: &Fees,
        _wallets: &[ScenarioWallet],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(
        &mut self,
        monitor: Monitor,
        _fees: Fees,
        wallets: Vec<ScenarioWallet>,
    ) -> anyhow::Result<Vec<ScenarioWallet>> {
        for i in 0..self.transfer_rounds {
            vlog::info!(
                "Fee ticker stressing cycle [{}/{}] started in {} mode",
                i + 1,
                self.transfer_rounds,
                if self.max_batch_size.is_some() {
                    "batch"
                } else {
                    "single"
                }
            );

            match self.max_batch_size {
                Some(max_batch_size) => {
                    self.process_txs_batch_transfer(&monitor, &wallets, max_batch_size)
                        .await
                }
                None => self.process_single_tx_transfer(&monitor, &wallets).await,
            }?
        }
        Ok(wallets)
    }

    async fn finalize(
        &mut self,
        _monitor: &Monitor,
        _fees: &Fees,
        _wallets: &[ScenarioWallet],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

impl FeeTickerScenario {
    pub fn new(token_name: TokenLike, config: FeeTickerScenarioConfig) -> Self {
        Self {
            token_name,
            transfer_size: gwei_to_wei(config.transfer_size),
            transfer_rounds: config.transfer_rounds,
            wallets: config.wallets_amount as usize,
            max_batch_size: config.max_batch_size.map(|x| x as usize),
        }
    }

    async fn process_single_tx_transfer(
        &self,
        monitor: &Monitor,
        wallets: &[ScenarioWallet],
    ) -> anyhow::Result<()> {
        wait_all_failsafe_chunks(
            "run/fee_ticker/single_tx_transfer",
            CHUNK_SIZES,
            wallets
                .iter()
                .map(|wallet| Self::send_transfer_to_self(monitor, &self.transfer_size, wallet))
                .collect::<Vec<_>>(),
        )
        .await?;
        Ok(())
    }

    async fn send_transfer_to_self(
        monitor: &Monitor,
        transfer_size: &BigUint,
        wallet: &ScenarioWallet,
    ) -> anyhow::Result<()> {
        let fee = wallet.sufficient_fee().await?;

        vlog::debug!(
            "Process transfer to self for {}: got fee {}",
            wallet.account_id().unwrap(),
            format_ether(&fee)
        );

        wallet.refresh_nonce().await?;
        let (tx, eth_signature) = wallet
            .sign_transfer(
                wallet.address(),
                closest_packable_token_amount(transfer_size),
                fee,
            )
            .await?;

        monitor.send_tx(tx, eth_signature).await?;
        Ok(())
    }

    async fn process_txs_batch_transfer(
        &self,
        monitor: &Monitor,
        wallets: &[ScenarioWallet],
        max_batch_size: usize,
    ) -> anyhow::Result<()> {
        let send_batch_task = DynamicChunks::new(wallets, batch_sizes_iter(max_batch_size))
            .map(|wallets| {
                let token = wallets[0].token_name().to_owned();
                let addresses = wallets.iter().map(|wallet| wallet.address()).collect();

                async move {
                    let total_fee = monitor
                        .provider
                        .get_txs_batch_fee(
                            vec![TxFeeTypes::Transfer; wallets.len()],
                            addresses,
                            token,
                        )
                        .await?;

                    vlog::debug!(
                        "Process batch transfers {}: got total fee {}",
                        wallets.len(),
                        format_ether(&total_fee)
                    );

                    // Refresh wallet nonces.
                    for wallet in &wallets {
                        wallet.refresh_nonce().await?;
                    }

                    let mut total_fee = Some(total_fee);
                    let sign_transfers_task = wallets
                        .iter()
                        .map(|wallet| {
                            let transfer_size = self.transfer_size.clone();
                            let fee = total_fee.take().unwrap_or_default();
                            async move {
                                wallet
                                    .sign_transfer(wallet.address(), transfer_size, fee)
                                    .await
                            }
                        })
                        .collect::<Vec<_>>();

                    let txs_signed = wait_all_failsafe_chunks(
                        "run/fee_ticker/prepare_batch",
                        CHUNK_SIZES,
                        sign_transfers_task,
                    )
                    .await?;
                    monitor.send_txs_batch(txs_signed).await?;
                    Ok(()) as anyhow::Result<()>
                }
            })
            .collect::<Vec<_>>();
        wait_all_failsafe("run/fee_ticker/batch_tx_transfer", send_batch_task).await?;

        Ok(())
    }
}
