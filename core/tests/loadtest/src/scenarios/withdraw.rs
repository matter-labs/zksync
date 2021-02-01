// Built-in uses
use std::fmt;
// External uses
use async_trait::async_trait;
use num::BigUint;
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync::{types::BlockStatus, utils::closest_packable_token_amount};
use zksync_types::TokenLike;
// Local uses
use super::{Fees, Scenario, ScenarioResources};
use crate::{monitor::Monitor, utils::wait_all_failsafe, wallet::ScenarioWallet};

/// Configuration options for the withdraw scenario.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct WithdrawScenarioConfig {
    /// Amount of intermediate wallets to use.
    pub wallets_amount: u64,
    /// Amount of "withdraw and deposit" iterations.
    pub withdraw_rounds: u64,
}

impl Default for WithdrawScenarioConfig {
    fn default() -> Self {
        Self {
            wallets_amount: 5,
            withdraw_rounds: 5,
        }
    }
}

/// Withdraw scenario performs several deposit / withdraw operations.
///
/// The purpose of the withdraw scenario is to ensure that deposits
/// and withdraws are processed correctly when the node is under a
/// load of many transfers.
#[derive(Debug)]
pub struct WithdrawScenario {
    token_name: TokenLike,
    config: WithdrawScenarioConfig,
}

impl fmt::Display for WithdrawScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "withdraw({})", self.token_name)
    }
}

#[async_trait]
impl Scenario for WithdrawScenario {
    fn requested_resources(&self, fees: &Fees) -> ScenarioResources {
        let balance_per_wallet =
            (&fees.zksync + &fees.eth) * (BigUint::from(self.config.withdraw_rounds));

        ScenarioResources {
            wallets_amount: self.config.wallets_amount,
            balance_per_wallet,
            token_name: self.token_name.clone(),
            has_deposits: true,
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
        fees: Fees,
        wallets: Vec<ScenarioWallet>,
    ) -> anyhow::Result<Vec<ScenarioWallet>> {
        for i in 0..self.config.withdraw_rounds {
            vlog::info!(
                "Withdraw and deposit cycle [{}/{}] started",
                i + 1,
                self.config.withdraw_rounds
            );

            let futures = wallets
                .iter()
                .map(|wallet| Self::withdraw_and_deposit(&monitor, &fees, wallet))
                .collect::<Vec<_>>();
            wait_all_failsafe(&format!("withdraw/run/cycle/{}", i), futures).await?;

            vlog::info!(
                "Withdraw and deposit cycle [{}/{}] finished",
                i + 1,
                self.config.withdraw_rounds
            );
        }

        vlog::info!("Withdraw scenario has been finished");

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

impl WithdrawScenario {
    pub fn new(token_name: TokenLike, config: WithdrawScenarioConfig) -> Self {
        Self { token_name, config }
    }

    async fn withdraw_and_deposit(
        monitor: &Monitor,
        fees: &Fees,
        wallet: &ScenarioWallet,
    ) -> anyhow::Result<()> {
        let amount = closest_packable_token_amount(
            &(wallet.balance(BlockStatus::Committed).await? - &fees.zksync),
        );

        let (tx, sign) = wallet
            .sign_withdraw(amount.clone(), fees.zksync.clone())
            .await?;
        monitor
            .wait_for_tx(BlockStatus::Verified, monitor.send_tx(tx, sign).await?)
            .await?;

        await_condition!(
            std::time::Duration::from_millis(1_00),
            wallet.l1_balance().await? >= amount
        );

        let balance = wallet.l1_balance().await?;
        anyhow::ensure!(balance > fees.eth, "Ethereum fee is too low");

        let amount = closest_packable_token_amount(&(balance - &fees.eth));
        monitor
            .wait_for_priority_op(BlockStatus::Verified, &wallet.deposit(amount).await?)
            .await?;

        Ok(())
    }
}
