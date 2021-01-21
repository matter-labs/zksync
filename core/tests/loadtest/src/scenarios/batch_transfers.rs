// Built-in uses
use std::fmt;
// External uses
use async_trait::async_trait;
use futures::future::try_join_all;
use num::BigUint;
use rand::{thread_rng, Rng};
use serde::{Deserialize, Serialize};
// Workspace uses
use zksync::utils::closest_packable_token_amount;
use zksync_types::{tx::PackedEthSignature, ZkSyncTx};
// Local uses
use super::{Fees, Scenario, ScenarioResources};
use crate::{
    monitor::Monitor,
    test_wallet::TestWallet,
    utils::{foreach_failsafe, gwei_to_wei, DynamicChunks},
};

/// Configuration options for the transfers scenario.
#[derive(Debug, Serialize, Deserialize, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct BatchTransferScenarioConfig {
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

impl Default for BatchTransferScenarioConfig {
    fn default() -> Self {
        Self {
            transfer_size: 1,
            transfer_rounds: 10,
            wallets_amount: 100,
            max_batch_size: 5,
        }
    }
}

impl From<BatchTransferScenarioConfig> for BatchTransferScenario {
    fn from(cfg: BatchTransferScenarioConfig) -> Self {
        Self::new(cfg)
    }
}

/// Schematically, scenario will look like this:
///
/// ```text
/// Deposit  | Transfer to new  | Transfer batches | Collect back | Withdraw to ETH
///          |                  |          |              |
///          |                  |          |              |
///          |           ┏━━━>Acc1━━━━━┓┗>Acc1━━━┓     |
///          |         ┏━┻━━━>Acc2━━━━┓┗━>Acc2━━━┻┓    |
/// ETH━━━━>InitialAcc━╋━━━━━>Acc3━━━┓┗━━>Acc3━━━━╋━>InitialAcc━>ETH
///          |         ┗━┳━━━>Acc4━━┓┗━━━>Acc4━━━┳┛    |
///          |           ┗━━━>Acc5━┓┗━━━━>Acc5━━━┛     |
/// ```
#[derive(Debug)]
pub struct BatchTransferScenario {
    transfer_size: BigUint,
    transfer_rounds: u64,
    wallets: u64,
    max_batch_size: usize,
    txs: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
}

impl BatchTransferScenario {
    pub fn new(config: BatchTransferScenarioConfig) -> Self {
        Self {
            transfer_size: gwei_to_wei(config.transfer_size),
            transfer_rounds: config.transfer_rounds,
            wallets: config.wallets_amount,
            max_batch_size: config.max_batch_size as usize,
            txs: Vec::new(),
        }
    }
}

impl fmt::Display for BatchTransferScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("batch_transfers")
    }
}

#[async_trait]
impl Scenario for BatchTransferScenario {
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
        fees: Fees,
        wallets: Vec<TestWallet>,
    ) -> anyhow::Result<Vec<TestWallet>> {
        let max_batch_size = self.max_batch_size;
        let batch_sizes = std::iter::repeat_with(move || match thread_rng().gen_range(0, 3) {
            0 => 2,
            1 => max_batch_size / 2,
            2 => max_batch_size,
            _ => unreachable!(),
        });

        let transfers_number = (self.wallets * self.transfer_rounds) as usize;
        let txs = (0..transfers_number).map(|i| {
            let from = i % wallets.len();
            let to = (i + 1) % wallets.len();
            (from, to)
        });

        let txs_task = DynamicChunks::new(txs, batch_sizes).map(|txs| {
            let txs_task = try_join_all(txs.into_iter().map(|(from, to)| {
                wallets[from].sign_transfer(
                    wallets[to].address(),
                    closest_packable_token_amount(&self.transfer_size),
                    fees.zksync.clone(),
                )
            }));

            let monitor = monitor.clone();
            async move {
                let txs = txs_task.await?;
                eprintln!("Batch_transfers: sent_txs {}", txs.len());
                monitor.send_txs_batch(txs).await
            }
        });
        foreach_failsafe("run/batch_transfers", txs_task).await?;

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
