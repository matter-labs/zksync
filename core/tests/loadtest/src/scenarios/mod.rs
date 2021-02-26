//! Module with various scenarios for a `loadtest`.
//! A scenario is basically is a behavior policy for sending the transactions.
//! A simplest scenario will be: "get a bunch of accounts and just spawn a lot of transfer
//! operations between them".

// Public uses
pub use self::{
    full_exit::FullExitScenarioConfig, transfers::TransferScenarioConfig,
    withdraw::WithdrawScenarioConfig,
};

// Built-in uses
use std::{
    collections::BTreeMap,
    fmt::{Debug, Display},
};

// External uses
use async_trait::async_trait;
use batch_transfers::{BatchTransferScenario, BatchTransferScenarioConfig};
use fee_ticker::{FeeTickerScenario, FeeTickerScenarioConfig};
use num::BigUint;
use serde::{Deserialize, Serialize};
use zksync_types::TokenLike;
// Workspace uses

// Local uses
use self::{full_exit::FullExitScenario, transfers::TransferScenario, withdraw::WithdrawScenario};
use crate::{monitor::Monitor, wallet::ScenarioWallet, FiveSummaryStats};

mod batch_transfers;
mod fee_ticker;
mod full_exit;
mod transfers;
mod withdraw;

/// Resources that are needed from the scenario executor to perform the scenario.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ScenarioResources {
    /// Total amount of non-empty wallets.
    pub wallets_amount: u64,
    /// Wei balance in each wallet.
    pub balance_per_wallet: BigUint,
    /// Scenario token.
    pub token_name: TokenLike,
    /// Scenario has deposit operations.
    pub has_deposits: bool,
}

/// Sufficient fee for the related type of transaction.
#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct Fees {
    /// Fee for the Ethereum transactions.
    pub eth: BigUint,
    /// Fee for the zkSync transactions.
    pub zksync: BigUint,
}

/// Describes the general steps of a load test scenario.
#[async_trait]
pub trait Scenario: Debug + Display + Send + Sync + 'static {
    /// Returns resources that should be provided by the scenario executor.
    fn requested_resources(&self, fees: &Fees) -> ScenarioResources;

    /// Performs actions before running the main scenario, for example, it can
    /// fill the queue of transactions for execution.
    async fn prepare(
        &mut self,
        monitor: &Monitor,
        fees: &Fees,
        wallets: &[ScenarioWallet],
    ) -> anyhow::Result<()>;

    /// Runs main scenario routine with the enabled load monitor.
    async fn run(
        &mut self,
        monitor: Monitor,
        fees: Fees,
        wallets: Vec<ScenarioWallet>,
    ) -> anyhow::Result<Vec<ScenarioWallet>>;

    /// Performs actions after running the main scenario, for example, it can
    /// return the funds to the specified wallets.
    async fn finalize(
        &mut self,
        monitor: &Monitor,
        fees: &Fees,
        wallets: &[ScenarioWallet],
    ) -> anyhow::Result<()>;
}

/// Supported scenario types.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
#[serde(tag = "name", rename_all = "snake_case")]
pub enum ScenarioType {
    /// Bunch of transfers scenario.
    Transfer(TransferScenarioConfig),
    /// Withdraw / deposit scenario.
    Withdraw(WithdrawScenarioConfig),
    /// Full exit / deposit scenario.
    FullExit(FullExitScenarioConfig),
    /// Batched transfers scenario.
    BatchTransfers(BatchTransferScenarioConfig),
    /// Stressing fee ticker scenario.
    FeeTicker(FeeTickerScenarioConfig),
}

/// Scenario config.
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct ScenarioConfig {
    /// Scenario token name.
    pub token_name: TokenLike,
    /// Fee for the zkSync transactions in gwei.
    #[serde(default)]
    pub zksync_fee: Option<u64>,
    /// Inner scenario config.
    #[serde(flatten)]
    pub inner: ScenarioType,
}

impl ScenarioConfig {
    /// Returns the scenario given its type.
    pub fn into_scenario(self) -> Box<dyn Scenario> {
        match self.inner {
            ScenarioType::Transfer(cfg) => Box::new(TransferScenario::new(self.token_name, cfg)),
            ScenarioType::Withdraw(cfg) => Box::new(WithdrawScenario::new(self.token_name, cfg)),
            ScenarioType::FullExit(cfg) => Box::new(FullExitScenario::new(self.token_name, cfg)),
            ScenarioType::BatchTransfers(cfg) => {
                Box::new(BatchTransferScenario::new(self.token_name, cfg))
            }
            ScenarioType::FeeTicker(cfg) => Box::new(FeeTickerScenario::new(self.token_name, cfg)),
        }
    }
}

/// Load test report for the each transaction variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxVariantTestsReport {
    /// A five numbers summary statistic for each transaction lifecycle step.
    pub stats: BTreeMap<String, Option<FiveSummaryStats>>,
}

/// Load test report for the transactions scenarios.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScenariosTestsReport {
    /// A five numbers summary statistic for each transaction variant.
    pub summary: BTreeMap<String, TxVariantTestsReport>,
    /// Total amount of sent requests.
    pub total_txs_count: usize,
    /// Amount of failed requests regardless of the cause of the failure.
    pub failed_txs_count: usize,
}
