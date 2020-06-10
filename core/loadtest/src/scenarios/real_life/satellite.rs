//! Satellite scenario for real-life loadtest.
//!
//! Satellite scenario is ran concurrently to the main scenario
//! and it performs several deposit / withdraw operations at the same
//! time as the funds are rotated in the main scenario.
//!
//! The purpose of the satellite scenario is to ensure that deposits
//! and withdraws are processed correctly when the node is under a
//! load of many transfers.

// Built-in deps
use std::time::Duration;
// External deps
use num::BigUint;
// Workspace deps
// Local deps
use crate::{rpc_client::RpcClient, test_accounts::TestAccount};

#[derive(Debug)]
pub struct SatelliteScenario {
    rpc_client: RpcClient,
    accounts: Vec<TestAccount>,
    deposit_size: BigUint,
    verify_timeout: Duration,
    estimated_fee_for_op: BigUint,
}

impl SatelliteScenario {
    pub fn new(
        rpc_client: RpcClient,
        accounts: Vec<TestAccount>,
        deposit_size: BigUint,
        verify_timeout: Duration,
    ) -> Self {
        Self {
            rpc_client,
            accounts,
            deposit_size,
            verify_timeout,
            estimated_fee_for_op: 0u32.into(),
        }
    }

    pub fn set_estimated_fee(&mut self, estimated_fee_for_op: BigUint) {
        self.estimated_fee_for_op = estimated_fee_for_op
    }

    pub async fn run(&mut self) -> Result<(), failure::Error> {
        Ok(())
    }
}
