// Built-in uses
use std::fmt;

// External uses
use async_trait::async_trait;
use num::BigUint;
use serde::{Deserialize, Serialize};

// Workspace uses
use zksync::utils::closest_packable_token_amount;
use zksync_utils::format_ether;

// Local uses
use super::{Fees, Scenario, ScenarioResources};
use crate::{
    monitor::Monitor,
    test_wallet::TestWallet,
    utils::{gwei_to_wei, wait_all_failsafe_chunks, CHUNK_SIZES},
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
    pub max_batch_size: u64,
}

impl Default for FeeTickerScenarioConfig {
    fn default() -> Self {
        Self {
            transfer_size: 1,
            transfer_rounds: 10,
            wallets_amount: 100,
            max_batch_size: 50,
        }
    }
}

impl From<FeeTickerScenarioConfig> for FeeTickerScenario {
    fn from(cfg: FeeTickerScenarioConfig) -> Self {
        Self::new(cfg)
    }
}

#[derive(Debug)]
pub struct FeeTickerScenario {
    transfer_size: BigUint,
    transfer_rounds: u64,
    wallets: u64,
    max_batch_size: usize,
}

impl fmt::Display for FeeTickerScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("fee_ticker")
    }
}

#[async_trait]
impl Scenario for FeeTickerScenario {
    fn requested_resources(&self, fees: &Fees) -> ScenarioResources {
        let balance_per_wallet =
            &self.transfer_size + (&fees.zksync * BigUint::from(self.transfer_rounds));

        ScenarioResources {
            balance_per_wallet: closest_packable_token_amount(&balance_per_wallet),
            wallets_amount: self.wallets,
            has_deposits: false,
        }
    }

    async fn prepare(
        &mut self,
        _monitor: &Monitor,
        _fees: &Fees,
        _wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn run(
        &mut self,
        monitor: Monitor,
        _fees: Fees,
        wallets: Vec<TestWallet>,
    ) -> anyhow::Result<Vec<TestWallet>> {
        for i in 0..self.transfer_rounds {
            vlog::info!(
                "Fee ticker stressing cycle [{}/{}] started",
                i + 1,
                self.transfer_rounds,
            );

            wait_all_failsafe_chunks(
                "run/single_tx_transfer",
                CHUNK_SIZES,
                (0..self.wallets as usize).map(|i| {
                    let from = &wallets[i % wallets.len()];
                    let to = &wallets[(i + 1) % wallets.len()];

                    self.process_single_tx_transfer(&monitor, from, to)
                }),
            )
            .await?;
        }
        Ok(wallets)
    }

    async fn finalize(
        &mut self,
        _monitor: &Monitor,
        _fees: &Fees,
        _wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        Ok(())
    }
}

impl FeeTickerScenario {
    pub fn new(config: FeeTickerScenarioConfig) -> Self {
        Self {
            transfer_size: gwei_to_wei(config.transfer_size),
            transfer_rounds: config.transfer_rounds,
            wallets: config.wallets_amount,
            max_batch_size: config.max_batch_size as usize,
        }
    }

    pub async fn process_single_tx_transfer(
        &self,
        monitor: &Monitor,
        from: &TestWallet,
        to: &TestWallet,
    ) -> anyhow::Result<()> {
        let fee = from.sufficient_fee().await?;

        vlog::debug!(
            "Process transfer from {} to {}: got fee {}",
            from.account_id().unwrap(),
            to.account_id().unwrap(),
            format_ether(&fee)
        );

        let (tx, eth_signature) = from
            .sign_transfer(
                to.address(),
                closest_packable_token_amount(&self.transfer_size),
                fee,
            )
            .await?;

        monitor.send_tx(tx, eth_signature).await?;
        Ok(())
    }
}
