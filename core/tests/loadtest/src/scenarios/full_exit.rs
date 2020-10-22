// Built-in uses
use std::fmt;
// External uses
use async_trait::async_trait;
use num::BigUint;
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync::{types::BlockStatus, utils::closest_packable_token_amount};
// Local uses
use super::{Scenario, ScenarioResources};
use crate::{monitor::Monitor, test_wallet::TestWallet, utils::try_wait_all};

/// Configuration options for the full exit scenario.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct FullExitScenarioConfig {
    /// Amount of intermediate wallets to use.
    pub wallets_amount: u64,
}

impl Default for FullExitScenarioConfig {
    fn default() -> Self {
        Self { wallets_amount: 5 }
    }
}

impl From<FullExitScenarioConfig> for FullExitScenario {
    fn from(config: FullExitScenarioConfig) -> Self {
        Self { config }
    }
}

#[derive(Debug)]
pub struct FullExitScenario {
    config: FullExitScenarioConfig,
}

impl fmt::Display for FullExitScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("full_exit")
    }
}

#[async_trait]
impl Scenario for FullExitScenario {
    fn requested_resources(&self, sufficient_fee: &BigUint) -> ScenarioResources {
        let balance_per_wallet = sufficient_fee * (BigUint::from(4_u64));

        ScenarioResources {
            wallets_amount: self.config.wallets_amount,
            balance_per_wallet,
        }
    }

    async fn prepare(
        &mut self,
        monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        for wallet in wallets {
            // Withdraw some amount to have enough funds to perform `full_exit` operation.
            let withdraw_amount =
                closest_packable_token_amount(&(sufficient_fee * BigUint::from(4_u64)));

            let (tx, sign) = wallet
                .sign_withdraw(withdraw_amount.clone(), sufficient_fee.clone())
                .await?;
            monitor
                .wait_for_tx(BlockStatus::Verified, monitor.send_tx(tx, sign).await?)
                .await?;

            await_condition!(std::time::Duration::from_millis(1_00), {
                wallet.eth_balance().await? >= withdraw_amount
            });
        }

        Ok(())
    }

    async fn run(
        &mut self,
        monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        log::info!("Full exit and deposit cycle started");

        let futures = wallets
            .iter()
            .map(|wallet| Self::full_exit_and_deposit(monitor, sufficient_fee, wallet))
            .collect::<Vec<_>>();
        try_wait_all(futures).await?;

        log::info!("Full exit scenario has been finished");

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

impl FullExitScenario {
    async fn full_exit_and_deposit(
        monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallet: &TestWallet,
    ) -> anyhow::Result<()> {
        monitor
            .wait_for_priority_op(BlockStatus::Verified, &wallet.full_exit().await?)
            .await?;

        let amount = closest_packable_token_amount(
            &(wallet.eth_balance().await? - sufficient_fee * BigUint::from(2_u64)),
        );
        monitor
            .wait_for_priority_op(BlockStatus::Committed, &wallet.deposit(amount).await?)
            .await?;

        Ok(())
    }
}
