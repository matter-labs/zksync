//! Real-life loadtest scenario does not measure the TPS nor simulated the high load,
//! but rather simulates the real-life use case of zkSync:
//!
//! 1. Funds are deposited from one Ethereum account into one new zkSync account.
//! 2. Once funds are deposited, this account split the funds between N accounts
//!    using the `transferToNew` operation.
//! 3. Once funds are transferred and verified, these funds are "rotated" within
//!    created accounts using the `transfer` operation. This operation is repeated
//!    M times.
//! 4. To finish the test, all the funds are collected back to the initial account
//!    are withdrawn to the Ethereum.
//!
//! `N` and `M` are configurable parameters, meaning the breadth of the test (how
//! many accounts will be used within the test) and the depth of the test (how
//! many rotation cycles are performed) correspondingly.
//!
//! Schematically, scenario will look like this:
//!
//! Deposit  | Transfer to new  | Transfer | Collect back | Withdraw to ETH
//!
//! ```text
//!                                ┗━━━━┓
//!                      ┏━━━>Acc1━━━━━┓┗>Acc1━━━┓
//!                    ┏━┻━━━>Acc2━━━━┓┗━>Acc2━━━┻┓
//! ETH━━━━>InitialAcc━╋━━━━━>Acc3━━━┓┗━━>Acc3━━━━╋━>InitialAcc━>ETH
//!                    ┗━┳━━━>Acc4━━┓┗━━━>Acc4━━━┳┛
//!                      ┗━━━>Acc5━┓┗━━━━>Acc5━━━┛
//! ```

// Temporary, for development

#![allow(dead_code)]

// Built-in deps
// Local deps
use crate::{rpc_client::RpcClient, scenarios::ScenarioContext};

#[derive(Debug)]
enum TestPhase {
    Init,
    Deposit,
    InitialTransfer,
    FundsRotation,
    CollectingFunds,
    Withdraw,
}

#[derive(Debug)]
struct ScenarioExecutor {
    phase: TestPhase,
    rpc_client: RpcClient,
}

impl ScenarioExecutor {
    pub fn new(rpc_client: RpcClient) -> Self {
        Self {
            phase: TestPhase::Init,
            rpc_client,
        }
    }

    pub async fn run(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }
}

/// Runs the outgoing TPS scenario:
/// sends the different types of transactions, and measures the TPS for the sending
/// process (in other words, speed of the ZKSync node mempool).
pub fn run_scenario(mut ctx: ScenarioContext) {
    // let verify_timeout_sec = Duration::from_secs(ctx.ctx.verify_timeout_sec);
    let rpc_addr = ctx.rpc_addr.clone();

    let rpc_client = RpcClient::new(&rpc_addr);

    let mut scenario = ScenarioExecutor::new(rpc_client);

    // Obtain the Ethereum node JSON RPC address.
    log::info!("Starting the loadtest");

    // Run the scenario.
    log::info!("Waiting for all transactions to be verified");
    ctx.rt
        .block_on(scenario.run())
        .expect("Failed the scenario");
    log::info!("Loadtest completed.");
}
