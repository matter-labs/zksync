//! This module provides utilities for estimating the gas costs for
//! the transactions that server sends to the Ethereum network.
//! Server uses this module to ensure that generated transactions
//! won't run out of the gas and won't trespass the block gas limit.

// Workspace deps
use zksync_basic_types::U256;
// Local deps
use crate::{config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL, ZkSyncOp};

/// Amount of gas that we can afford to spend in one transaction.
/// This value must be big enough to fit big blocks with expensive transactions,
/// but at the same time it should not exceed the block gas limit.
pub const TX_GAS_LIMIT: u64 = 4_000_000;

#[derive(Debug)]
pub struct CommitCost;

impl CommitCost {
    // Below are costs of processing every kind of operation
    // in `commitBlock` contract call.
    //
    // These values are estimated using the `gas_price_test` in `testkit`.

    // TODO: overvalued for quick fix of tx fails
    pub const BASE_COST: u64 = 300_000;
    pub const DEPOSIT_COST: u64 = 10_397;
    pub const CHANGE_PUBKEY_COST_OFFCHAIN: u64 = 15_866;
    pub const CHANGE_PUBKEY_COST_ONCHAIN: u64 = 3_929;
    pub const TRANSFER_COST: u64 = 334;
    pub const TRANSFER_TO_NEW_COST: u64 = 862;
    pub const FULL_EXIT_COST: u64 = 10_165;
    pub const WITHDRAW_COST: u64 = 2_167;
    pub const FORCED_EXIT_COST: u64 = Self::WITHDRAW_COST; // TODO: Verify value

    pub fn base_cost() -> U256 {
        U256::from(Self::BASE_COST)
    }

    pub fn op_cost(op: &ZkSyncOp) -> U256 {
        let cost = match op {
            ZkSyncOp::Noop(_) => 0,
            ZkSyncOp::Deposit(_) => Self::DEPOSIT_COST,
            ZkSyncOp::ChangePubKeyOffchain(change_pubkey) => {
                if change_pubkey.tx.eth_signature.is_some() {
                    Self::CHANGE_PUBKEY_COST_OFFCHAIN
                } else {
                    Self::CHANGE_PUBKEY_COST_ONCHAIN
                }
            }
            ZkSyncOp::Transfer(_) => Self::TRANSFER_COST,
            ZkSyncOp::TransferToNew(_) => Self::TRANSFER_TO_NEW_COST,
            ZkSyncOp::FullExit(_) => Self::FULL_EXIT_COST,
            ZkSyncOp::Withdraw(_) => Self::WITHDRAW_COST,
            ZkSyncOp::ForcedExit(_) => Self::FORCED_EXIT_COST,
            ZkSyncOp::Close(_) => unreachable!("Close operations are disabled"),
        };

        U256::from(cost)
    }
}

#[derive(Debug)]
pub struct VerifyCost;

impl VerifyCost {
    // Below are costs of processing every kind of operation
    // in `verifyBlock` contract call.
    //
    // These values are estimated using the `gas_price_test` in `testkit`.

    // TODO: overvalued for quick fix of tx fails
    pub const BASE_COST: u64 = 1_000_000;
    pub const DEPOSIT_COST: u64 = 0;
    pub const CHANGE_PUBKEY_COST: u64 = 0;
    pub const TRANSFER_COST: u64 = 0;
    pub const TRANSFER_TO_NEW_COST: u64 = 0;
    pub const FULL_EXIT_COST: u64 = 2_499;
    pub const WITHDRAW_COST: u64 = 45_668;
    pub const FORCED_EXIT_COST: u64 = Self::WITHDRAW_COST; // TODO: Verify value

    pub fn base_cost() -> U256 {
        U256::from(Self::BASE_COST)
    }

    pub fn op_cost(op: &ZkSyncOp) -> U256 {
        let cost = match op {
            ZkSyncOp::Noop(_) => 0,
            ZkSyncOp::Deposit(_) => Self::DEPOSIT_COST,
            ZkSyncOp::ChangePubKeyOffchain(_) => Self::CHANGE_PUBKEY_COST,
            ZkSyncOp::Transfer(_) => Self::TRANSFER_COST,
            ZkSyncOp::TransferToNew(_) => Self::TRANSFER_TO_NEW_COST,
            ZkSyncOp::FullExit(_) => Self::FULL_EXIT_COST,
            ZkSyncOp::Withdraw(_) => Self::WITHDRAW_COST,
            ZkSyncOp::ForcedExit(_) => Self::FORCED_EXIT_COST,
            ZkSyncOp::Close(_) => unreachable!("Close operations are disabled"),
        };

        U256::from(cost)
    }
}

/// `GasCounter` is an entity capable of counting the estimated gas cost of an
/// upcoming transaction. It watches for the total gas cost of either commit
/// or withdraw operation to not exceed the reasonable gas limit amount.
/// It is used by `state_keeper` module to seal the block once we're not able
/// to safely insert any more transactions.
///
/// The estimation process is based on the pre-calculated "base cost" of operation
/// (basically, cost of processing an empty block), and the added cost of all the
/// operations in that block.
///
/// These estimated costs were calculated using the `gas_price_test` from `testkit`.
#[derive(Debug, Clone)]
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
    /// Base cost of `completeWithdrawals` contract method call.
    pub const COMPLETE_WITHDRAWALS_BASE_COST: u64 = 30_307;
    /// Cost of processing one withdraw operation in `completeWithdrawals` contract call.
    pub const COMPLETE_WITHDRAWALS_COST: u64 = 41_641;

    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the cost of the operation to the gas counter.
    ///
    /// Returns `Ok(())` if transaction fits, and returns `Err(())` if
    /// the block must be sealed without this transaction.
    pub fn add_op(&mut self, op: &ZkSyncOp) -> Result<(), ()> {
        let new_commit_cost = self.commit_cost + CommitCost::op_cost(op);
        if Self::scale_up(new_commit_cost) > U256::from(TX_GAS_LIMIT) {
            return Err(());
        }

        let new_verify_cost = self.verify_cost + VerifyCost::op_cost(op);
        if Self::scale_up(new_verify_cost) > U256::from(TX_GAS_LIMIT) {
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

    pub fn complete_withdrawals_gas_limit() -> U256 {
        // Currently we always complete a constant amount of withdrawals in the contract call, so the upper limit
        // is predictable.
        let approx_limit = U256::from(Self::COMPLETE_WITHDRAWALS_BASE_COST)
            + U256::from(MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL)
                * U256::from(Self::COMPLETE_WITHDRAWALS_COST);

        // We scale this value up nevertheless, just in case.
        Self::scale_up(approx_limit)
    }

    /// Increases the value by 30%.
    fn scale_up(value: U256) -> U256 {
        value * U256::from(130) / U256::from(100)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{operations::ChangePubKeyOp, tx::ChangePubKey};

    #[test]
    fn commit_cost() {
        let change_pubkey_op = ChangePubKeyOp {
            tx: ChangePubKey::new(
                1,
                Default::default(),
                Default::default(),
                0,
                Default::default(),
                Default::default(),
                None,
                None,
            ),
            account_id: 1,
        };

        // TODO add other operations to this test.

        let test_vector = vec![(
            ZkSyncOp::from(change_pubkey_op),
            CommitCost::CHANGE_PUBKEY_COST_ONCHAIN,
        )];

        for (op, expected_cost) in test_vector {
            assert_eq!(CommitCost::op_cost(&op), U256::from(expected_cost));
        }
    }

    #[test]
    fn verify_cost() {
        let change_pubkey_op = ChangePubKeyOp {
            tx: ChangePubKey::new(
                1,
                Default::default(),
                Default::default(),
                0,
                Default::default(),
                Default::default(),
                None,
                None,
            ),
            account_id: 1,
        };

        // TODO add other operations to this test.

        let test_vector = vec![(
            ZkSyncOp::from(change_pubkey_op),
            VerifyCost::CHANGE_PUBKEY_COST,
        )];

        for (op, expected_cost) in test_vector {
            assert_eq!(VerifyCost::op_cost(&op), U256::from(expected_cost));
        }
    }

    #[test]
    fn gas_counter() {
        let change_pubkey_op = ChangePubKeyOp {
            tx: ChangePubKey::new(
                1,
                Default::default(),
                Default::default(),
                0,
                Default::default(),
                Default::default(),
                None,
                None,
            ),
            account_id: 1,
        };
        let zksync_op = ZkSyncOp::from(change_pubkey_op);

        let mut gas_counter = GasCounter::new();

        assert_eq!(gas_counter.commit_cost, U256::from(CommitCost::BASE_COST));
        assert_eq!(gas_counter.verify_cost, U256::from(VerifyCost::BASE_COST));

        // Verify cost is 0, thus amount of operations is determined by the commit cost.
        let amount_ops_in_block = (U256::from(TX_GAS_LIMIT)
            - GasCounter::scale_up(gas_counter.commit_cost))
            / GasCounter::scale_up(U256::from(CommitCost::CHANGE_PUBKEY_COST_ONCHAIN));

        for _ in 0..amount_ops_in_block.as_u64() {
            gas_counter
                .add_op(&zksync_op)
                .expect("Gas limit was not reached, but op adding failed");
        }

        // Expected gas limit is (base_cost + n_ops * op_cost) * 1.3
        let expected_commit_limit = (U256::from(CommitCost::BASE_COST)
            + amount_ops_in_block * U256::from(CommitCost::CHANGE_PUBKEY_COST_ONCHAIN))
            * U256::from(130)
            / U256::from(100);
        let expected_verify_limit = (U256::from(VerifyCost::BASE_COST)
            + amount_ops_in_block * U256::from(VerifyCost::CHANGE_PUBKEY_COST))
            * U256::from(130)
            / U256::from(100);
        assert_eq!(gas_counter.commit_gas_limit(), expected_commit_limit);
        assert_eq!(gas_counter.verify_gas_limit(), expected_verify_limit);

        // Attempt to add one more operation (it should fail).
        gas_counter
            .add_op(&zksync_op)
            .expect_err("Able to add operation beyond the gas limit");

        // Check again that limit has not changed.
        assert_eq!(gas_counter.commit_gas_limit(), expected_commit_limit);
        assert_eq!(gas_counter.verify_gas_limit(), expected_verify_limit);
    }
}
