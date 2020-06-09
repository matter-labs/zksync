//! This module provides utilities for estimating the gas costs for
//! the transactions that server sends to the Ethereum network.
//! Server uses this module to ensure that generated transactions
//! won't run out of the gas and won't trespass the block gas limit.

// External deps
use web3::types::U256;
// Workspace deps
use models::node::FranklinOp;

/// Amount of gas that we can afford to spend in one transaction.
pub const TX_GAS_LIMIT: u64 = 3_000_000;

#[derive(Debug)]
pub struct CommitCost;

impl CommitCost {
    pub const BASE_COST: u64 = 146026;
    pub const DEPOSIT_COST: u64 = 10397;
    pub const CHANGE_PUBKEY_COST: u64 = 27449;
    pub const TRANSFER_COST: u64 = 334;
    pub const TRANSFER_TO_NEW_COST: u64 = 862;
    pub const FULL_EXIT_COST: u64 = 10165;
    pub const WITHDRAW_COST: u64 = 2167;

    pub fn base_cost() -> U256 {
        U256::from(Self::BASE_COST)
    }

    pub fn op_cost(op: &FranklinOp) -> U256 {
        let cost = match op {
            FranklinOp::Noop(_) => 0,
            FranklinOp::Deposit(_) => Self::DEPOSIT_COST,
            FranklinOp::ChangePubKeyOffchain(_) => Self::CHANGE_PUBKEY_COST,
            FranklinOp::Transfer(_) => Self::TRANSFER_COST,
            FranklinOp::TransferToNew(_) => Self::TRANSFER_TO_NEW_COST,
            FranklinOp::FullExit(_) => Self::FULL_EXIT_COST,
            FranklinOp::Withdraw(_) => Self::WITHDRAW_COST,
            FranklinOp::Close(_) => unreachable!("Close operations are disabled"),
        };

        U256::from(cost)
    }
}

#[derive(Debug)]
pub struct VerifyCost;

impl VerifyCost {
    pub const BASE_COST: u64 = 527451;
    pub const DEPOSIT_COST: u64 = 10997;
    pub const CHANGE_PUBKEY_COST: u64 = 0;
    pub const TRANSFER_COST: u64 = 0;
    pub const TRANSFER_TO_NEW_COST: u64 = 0;
    pub const FULL_EXIT_COST: u64 = 11151;
    pub const WITHDRAW_COST: u64 = 45668;

    pub fn base_cost() -> U256 {
        U256::from(Self::BASE_COST)
    }

    pub fn op_cost(op: &FranklinOp) -> U256 {
        let cost = match op {
            FranklinOp::Noop(_) => 0,
            FranklinOp::Deposit(_) => Self::DEPOSIT_COST,
            FranklinOp::ChangePubKeyOffchain(_) => Self::CHANGE_PUBKEY_COST,
            FranklinOp::Transfer(_) => Self::TRANSFER_COST,
            FranklinOp::TransferToNew(_) => Self::TRANSFER_TO_NEW_COST,
            FranklinOp::FullExit(_) => Self::FULL_EXIT_COST,
            FranklinOp::Withdraw(_) => Self::WITHDRAW_COST,
            FranklinOp::Close(_) => unreachable!("Close operations are disabled"),
        };

        U256::from(cost)
    }
}

#[derive(Debug)]
pub struct GasCounter {
    commit_cost: U256,
    verify_cost: U256,
}

impl Default for GasCounter {
    fn default() -> Self {
        Self {
            commit_cost: CommitCost::base_cost(),
            verify_cost: VerifyCost::base_cost(),
        }
    }
}

impl GasCounter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_op(&mut self, op: &FranklinOp) -> Result<(), ()> {
        let new_commit_cost = self.commit_cost + CommitCost::op_cost(op);
        if new_commit_cost > U256::from(TX_GAS_LIMIT) {
            return Err(());
        }

        let new_verify_cost = self.verify_cost + VerifyCost::op_cost(op);
        if new_verify_cost > U256::from(TX_GAS_LIMIT) {
            return Err(());
        }

        self.commit_cost = new_commit_cost;
        self.verify_cost = new_verify_cost;

        Ok(())
    }

    pub fn commit_gas_limit(&self) -> U256 {
        self.commit_cost * U256::from(130) / U256::from(100)
    }

    pub fn verify_gas_limit(&self) -> U256 {
        self.verify_cost * U256::from(130) / U256::from(100)
    }
}
