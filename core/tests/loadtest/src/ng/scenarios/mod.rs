pub use simple::{ScenarioExecutor, SimpleScenario};

// Built-in import
// External uses
use async_trait::async_trait;
use num::BigUint;
// Workspace uses
// Local uses
use crate::{monitor::Monitor, test_accounts::TestWallet};

mod simple;

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ScenarioResources {
    pub wallets_amount: u64,
    pub balance_per_wallet: BigUint,
}

#[async_trait]
pub trait Scenario: std::fmt::Debug {
    fn name(&self) -> &str;

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
