// Built-in uses
use std::fmt;
// External uses
use async_trait::async_trait;
use num::BigUint;
// Workspace uses
use zksync::utils::closest_packable_token_amount;
use zksync_types::{tx::PackedEthSignature, ZkSyncTx};
// Local uses
use super::{Scenario, ScenarioResources};
use crate::{monitor::Monitor, test_wallet::TestWallet, utils::try_wait_all};

/// Schematically, scenario will look like this:
///
/// ```text
/// Deposit  | Transfer to new  | Transfer | Collect back | Withdraw to ETH
///          |                  |          |              |
///          |                  |          |              |
///          |           ┏━━━>Acc1━━━━━┓┗>Acc1━━━┓     |
///          |         ┏━┻━━━>Acc2━━━━┓┗━>Acc2━━━┻┓    |
/// ETH━━━━>InitialAcc━╋━━━━━>Acc3━━━┓┗━━>Acc3━━━━╋━>InitialAcc━>ETH
///          |         ┗━┳━━━>Acc4━━┓┗━━━>Acc4━━━┳┛    |
///          |           ┗━━━>Acc5━┓┗━━━━>Acc5━━━┛     |
/// ```
#[derive(Debug)]
pub struct TransferScenario {
    transfer_size: BigUint,
    transfer_rounds: u64,
    wallets: u64,
    txs: Vec<(ZkSyncTx, Option<PackedEthSignature>)>,
}

impl Default for TransferScenario {
    fn default() -> Self {
        Self {
            transfer_size: BigUint::from(1_000_u64),
            transfer_rounds: 100,
            wallets: 100,
            txs: Vec::new(),
        }
    }
}

impl fmt::Display for TransferScenario {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Transfers")
    }
}

#[async_trait]
impl Scenario for TransferScenario {
    fn requested_resources(&self, fee: &BigUint) -> ScenarioResources {
        let balance_per_wallet = &self.transfer_size + (fee * BigUint::from(self.transfer_rounds));

        ScenarioResources {
            balance_per_wallet: closest_packable_token_amount(&balance_per_wallet),
            wallets_amount: self.wallets,
        }
    }

    async fn prepare(
        &mut self,
        _monitor: &Monitor,
        sufficient_fee: &BigUint,
        wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        let transfers_number = (self.wallets * self.transfer_rounds) as usize;

        log::info!(
            "All the initial transfers have been verified, creating {} transactions \
            for the transfers step",
            transfers_number
        );

        self.txs = try_wait_all((0..transfers_number).map(|i| {
            let from = i % wallets.len();
            let to = (i + 1) % wallets.len();

            wallets[from].sign_transfer(
                wallets[to].address(),
                closest_packable_token_amount(&self.transfer_size),
                sufficient_fee.clone(),
            )
        }))
        .await?;

        log::info!("Created {} transactions...", self.txs.len());

        Ok(())
    }

    async fn run(
        &mut self,
        monitor: &Monitor,
        _sufficient_fee: &BigUint,
        _wallets: &[TestWallet],
    ) -> anyhow::Result<()> {
        try_wait_all(
            self.txs
                .drain(..)
                .map(|(tx, sign)| monitor.send_tx(tx, sign)),
        )
        .await?;

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
