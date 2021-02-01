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

#[derive(Debug)]
pub struct FullExitScenario {
    token_name: TokenLike,
    config: FullExitScenarioConfig,
}

impl fmt::Display for FullExitScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "full_exit({})", self.token_name)
    }
}

fn balance_per_wallet(fees: &Fees) -> BigUint {
    &fees.eth * BigUint::from(4_u64)
}

#[async_trait]
impl Scenario for FullExitScenario {
    fn requested_resources(&self, fees: &Fees) -> ScenarioResources {
        let balance_per_wallet = balance_per_wallet(fees);

        ScenarioResources {
            wallets_amount: self.config.wallets_amount,
            balance_per_wallet,
            token_name: self.token_name.clone(),
            has_deposits: true,
        }
    }

    async fn prepare(
        &mut self,
        monitor: &Monitor,
        fees: &Fees,
        wallets: &[ScenarioWallet],
    ) -> anyhow::Result<()> {
        // Withdraw some amount to have enough funds to perform `full_exit` operation.
        let withdraw_amount = closest_packable_token_amount(&balance_per_wallet(fees));
        let mut txs_queue = Vec::with_capacity(wallets.len());
        for wallet in wallets {
            let (tx, sign) = wallet
                .sign_withdraw(withdraw_amount.clone(), fees.zksync.clone())
                .await?;

            let tx_hash = monitor.send_tx(tx, sign).await?;
            txs_queue.push(monitor.wait_for_tx(BlockStatus::Verified, tx_hash));
        }
        wait_all_failsafe("full_exit/prepare", txs_queue.into_iter()).await?;

        vlog::info!("All withdrawal transactions have been verified");

        Ok(())
    }

    async fn run(
        &mut self,
        monitor: Monitor,
        fees: Fees,
        wallets: Vec<ScenarioWallet>,
    ) -> anyhow::Result<Vec<ScenarioWallet>> {
        vlog::info!("Full exit and deposit cycle started");

        let full_exit_task = wallets
            .iter()
            .map(|wallet| Self::full_exit_and_deposit(&monitor, &fees, wallet))
            .collect::<Vec<_>>();
        wait_all_failsafe("full_exit/run", full_exit_task).await?;

        vlog::info!("Full exit scenario has been finished");

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

impl FullExitScenario {
    pub fn new(token_name: TokenLike, config: FullExitScenarioConfig) -> Self {
        Self { token_name, config }
    }

    async fn full_exit_and_deposit(
        monitor: &Monitor,
        fees: &Fees,
        wallet: &ScenarioWallet,
    ) -> anyhow::Result<()> {
        monitor
            .wait_for_priority_op(BlockStatus::Verified, &wallet.full_exit().await?)
            .await?;

        let balance = wallet.l1_balance().await?;
        let amount = closest_packable_token_amount(&(balance - &fees.eth));
        monitor
            .wait_for_priority_op(BlockStatus::Committed, &wallet.deposit(amount).await?)
            .await?;

        Ok(())
    }
}
