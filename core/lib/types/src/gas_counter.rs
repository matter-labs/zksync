//! This module provides utilities for estimating the gas costs for
//! the transactions that server sends to the Ethereum network.
//! Server uses this module to ensure that generated transactions
//! won't run out of the gas and won't trespass the block gas limit.
// Workspace deps
use zksync_basic_types::*;
// Local deps
use crate::{config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL, Block, ZkSyncOp};

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

    // TODO: overvalued for quick fix of tx fails (ZKS-109).
    pub const BASE_COST: u64 = 51_000;
    pub const DEPOSIT_COST: u64 = 7_700;
    // TODO: estimate after changepubkey gas cost estimation is fixed [ZKS-554]
    pub const OLD_CHANGE_PUBKEY_COST_OFFCHAIN: u64 = 25_000;
    pub const CHANGE_PUBKEY_COST_OFFCHAIN: u64 = 12_700;
    pub const CHANGE_PUBKEY_COST_ONCHAIN: u64 = 6_400;
    pub const CHANGE_PUBKEY_COST_CREATE2: u64 = 8_450;
    pub const TRANSFER_COST: u64 = 300;
    pub const TRANSFER_TO_NEW_COST: u64 = 940;
    pub const FULL_EXIT_COST: u64 = 10_000;
    pub const WITHDRAW_COST: u64 = 3_900;
    pub const WITHDRAW_NFT_COST: u64 = 5_150;
    pub const FORCED_EXIT_COST: u64 = Self::WITHDRAW_COST; // TODO: Verify value (ZKS-109).
    pub const MINT_TOKEN_COST: u64 = 920;
    pub const SWAP_COST: u64 = 710;

    pub fn base_cost() -> U256 {
        U256::from(Self::BASE_COST)
    }

    pub fn op_cost(op: &ZkSyncOp) -> U256 {
        let cost = match op {
            ZkSyncOp::Noop(_) => 0,
            ZkSyncOp::Deposit(_) => Self::DEPOSIT_COST,
            ZkSyncOp::ChangePubKeyOffchain(_change_pubkey) => {
                Self::OLD_CHANGE_PUBKEY_COST_OFFCHAIN
                // TODO: Restore when we figure out why this failed [ZKS-554]
                // match change_pubkey.tx.get_change_pubkey_fee_type() {
                //     ChangePubKeyFeeTypeArg::ContractsV4Version(ChangePubKeyType::ECDSA) => {
                //         Self::CHANGE_PUBKEY_COST_OFFCHAIN
                //     }
                //     ChangePubKeyFeeTypeArg::ContractsV4Version(ChangePubKeyType::Onchain)
                //     | ChangePubKeyFeeTypeArg::PreContracts4Version {
                //         onchain_pubkey_auth: true,
                //     } => Self::CHANGE_PUBKEY_COST_ONCHAIN,
                //     ChangePubKeyFeeTypeArg::ContractsV4Version(ChangePubKeyType::CREATE2) => {
                //         Self::CHANGE_PUBKEY_COST_CREATE2
                //     }
                //     ChangePubKeyFeeTypeArg::PreContracts4Version {
                //         onchain_pubkey_auth: false,
                //     } => Self::OLD_CHANGE_PUBKEY_COST_OFFCHAIN,
                // }
            }
            ZkSyncOp::Transfer(_) => Self::TRANSFER_COST,
            ZkSyncOp::TransferToNew(_) => Self::TRANSFER_TO_NEW_COST,
            ZkSyncOp::FullExit(_) => Self::FULL_EXIT_COST,
            ZkSyncOp::Withdraw(_) => Self::WITHDRAW_COST,
            ZkSyncOp::ForcedExit(_) => Self::FORCED_EXIT_COST,
            ZkSyncOp::Swap(_) => Self::SWAP_COST,
            ZkSyncOp::MintNFTOp(_) => Self::MINT_TOKEN_COST,
            ZkSyncOp::Close(_) => unreachable!("Close operations are disabled"),
            ZkSyncOp::WithdrawNFT(_) => Self::WITHDRAW_NFT_COST,
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

    // TODO: overvalued for quick fix of tx fails (ZKS-109).
    pub const BASE_COST: u64 = 10_000;
    pub const DEPOSIT_COST: u64 = 100;
    pub const CHANGE_PUBKEY_COST: u64 = 0;
    pub const TRANSFER_COST: u64 = 0;
    pub const TRANSFER_TO_NEW_COST: u64 = 0;
    pub const SWAP_COST: u64 = 0;
    pub const FULL_EXIT_COST: u64 = 30_000;
    pub const WITHDRAW_COST: u64 = 48_000;
    pub const FORCED_EXIT_COST: u64 = Self::WITHDRAW_COST; // TODO: Verify value (ZKS-109).
    pub const MINT_NFT_COST: u64 = 0;
    pub const WITHDRAW_NFT_COST: u64 = 200_000;

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
            ZkSyncOp::MintNFTOp(_) => Self::MINT_NFT_COST,
            ZkSyncOp::Swap(_) => Self::SWAP_COST,
            ZkSyncOp::Close(_) => unreachable!("Close operations are disabled"),
            ZkSyncOp::WithdrawNFT(_) => Self::WITHDRAW_NFT_COST,
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

#[derive(Debug)]
pub struct WrongTransaction;

impl std::fmt::Display for WrongTransaction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Wrong transaction in gas counter")
    }
}

impl std::error::Error for WrongTransaction {}

impl GasCounter {
    /// Base cost of `completeWithdrawals` contract method call.
    pub const COMPLETE_WITHDRAWALS_BASE_COST: u64 = 30_307;
    /// Cost of processing one withdraw operation in `completeWithdrawals` contract call.
    pub const COMPLETE_WITHDRAWALS_COST: u64 = 41_641;
    /// Some ERС20 tokens may require a lot of gas to withdrawals.
    pub const COMPLETE_WITHDRAWALS_ERC20_COST: u64 = 200_000;

    /// constants for gas limit calculation of aggregated operations
    pub const BASE_COMMIT_BLOCKS_TX_COST: usize = 450_000;
    pub const BASE_EXECUTE_BLOCKS_TX_COST: usize = 450_000;
    pub const BASE_PROOF_BLOCKS_TX_COST: usize = 1_500_000;

    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the cost of the operation to the gas counter.
    ///
    /// Returns `Ok(())` if transaction fits, and returns `Err(())` if
    /// the block must be sealed without this transaction.
    pub fn add_op(&mut self, op: &ZkSyncOp) -> Result<(), WrongTransaction> {
        let new_commit_cost = self.commit_cost + CommitCost::op_cost(op);
        if Self::scale_up(new_commit_cost) > U256::from(TX_GAS_LIMIT) {
            return Err(WrongTransaction);
        }

        let new_verify_cost = self.verify_cost + VerifyCost::op_cost(op);
        if Self::scale_up(new_verify_cost) > U256::from(TX_GAS_LIMIT) {
            return Err(WrongTransaction);
        }

        self.commit_cost = new_commit_cost;
        self.verify_cost = new_verify_cost;

        Ok(())
    }

    pub fn can_include(&self, ops: &[ZkSyncOp]) -> bool {
        let ops_cost: (U256, U256) = ops
            .iter()
            .map(|op| (CommitCost::op_cost(op), VerifyCost::op_cost(op)))
            .fold((U256::zero(), U256::zero()), |mut sum, val| {
                sum.0 += val.0;
                sum.1 += val.1;
                sum
            });
        let new_commit_cost = self.commit_cost + ops_cost.0;
        let new_verify_cost = self.verify_cost + ops_cost.1;

        if Self::scale_up(new_commit_cost) > U256::from(TX_GAS_LIMIT) {
            return false;
        }
        if Self::scale_up(new_verify_cost) > U256::from(TX_GAS_LIMIT) {
            return false;
        }

        true
    }

    pub fn batch_fits_into_empty_block(ops: &[ZkSyncOp]) -> bool {
        Self::new().can_include(ops)
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
                * U256::from(Self::COMPLETE_WITHDRAWALS_ERC20_COST);

        // We scale this value up nevertheless, just in case.
        Self::scale_up(approx_limit)
    }

    pub fn commit_gas_limit_aggregated(blocks: &[Block]) -> U256 {
        let approx_limit = U256::from(Self::BASE_COMMIT_BLOCKS_TX_COST)
            + blocks
                .iter()
                .fold(U256::zero(), |acc, block| acc + block.commit_gas_limit);

        Self::scale_up(approx_limit)
    }

    pub fn execute_gas_limit_aggregated(blocks: &[Block]) -> U256 {
        let approx_limit = U256::from(Self::BASE_EXECUTE_BLOCKS_TX_COST)
            + blocks
                .iter()
                .fold(U256::zero(), |acc, block| acc + block.verify_gas_limit);

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
    use crate::{
        operations::{
            ChangePubKeyOp, DepositOp, ForcedExitOp, FullExitOp, NoopOp, TransferOp,
            TransferToNewOp, WithdrawOp,
        },
        priority_ops::{Deposit, FullExit},
        tx::{ChangePubKey, ForcedExit, Transfer, Withdraw},
        MintNFT, MintNFTOp, WithdrawNFT, WithdrawNFTOp,
    };

    #[test]
    fn commit_and_verify_cost() {
        let change_pubkey_op = ChangePubKeyOp {
            tx: ChangePubKey::new(
                AccountId(1),
                Default::default(),
                Default::default(),
                TokenId(0),
                Default::default(),
                Default::default(),
                Default::default(),
                None,
                None,
            ),
            account_id: AccountId(1),
        };
        let deposit_op = DepositOp {
            priority_op: Deposit {
                from: Default::default(),
                token: TokenId(0),
                amount: Default::default(),
                to: Default::default(),
            },
            account_id: AccountId(1),
        };
        let transfer_op = TransferOp {
            tx: Transfer::new(
                AccountId(1),
                Default::default(),
                Default::default(),
                TokenId(0),
                Default::default(),
                Default::default(),
                Nonce(0),
                Default::default(),
                None,
            ),
            from: AccountId(1),
            to: AccountId(1),
        };
        let transfer_to_new_op = TransferToNewOp {
            tx: Transfer::new(
                AccountId(1),
                Default::default(),
                Default::default(),
                TokenId(0),
                Default::default(),
                Default::default(),
                Nonce(0),
                Default::default(),
                None,
            ),
            from: AccountId(1),
            to: AccountId(1),
        };
        let noop_op = NoopOp {};
        let full_exit_op = FullExitOp {
            priority_op: FullExit {
                account_id: AccountId(0),
                eth_address: Default::default(),
                token: TokenId(0),
            },
            withdraw_amount: None,
            creator_account_id: None,
            creator_address: None,
            serial_id: None,
            content_hash: None,
        };
        let forced_exit_op = ForcedExitOp {
            tx: ForcedExit::new(
                AccountId(1),
                Default::default(),
                TokenId(0),
                Default::default(),
                Nonce(0),
                Default::default(),
                None,
            ),
            target_account_id: AccountId(1),
            withdraw_amount: None,
        };
        let withdraw_op = WithdrawOp {
            tx: Withdraw::new(
                AccountId(1),
                Default::default(),
                Default::default(),
                TokenId(0),
                Default::default(),
                Default::default(),
                Nonce(0),
                Default::default(),
                None,
            ),
            account_id: AccountId(1),
        };

        let withdraw_nft_op = WithdrawNFTOp {
            tx: WithdrawNFT::new(
                AccountId(1),
                Default::default(),
                Default::default(),
                TokenId(0),
                Default::default(),
                Default::default(),
                Nonce(0),
                Default::default(),
                None,
            ),
            creator_id: Default::default(),
            creator_address: Default::default(),
            content_hash: Default::default(),
            serial_id: 0,
        };

        let mint_nft_op = MintNFTOp {
            tx: MintNFT::new(
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                None,
            ),
            creator_account_id: Default::default(),
            recipient_account_id: Default::default(),
        };
        let test_vector_commit = vec![
            (
                ZkSyncOp::from(change_pubkey_op.clone()),
                CommitCost::OLD_CHANGE_PUBKEY_COST_OFFCHAIN, // restore after [ZKS-554]
            ),
            (ZkSyncOp::from(deposit_op.clone()), CommitCost::DEPOSIT_COST),
            (
                ZkSyncOp::from(transfer_op.clone()),
                CommitCost::TRANSFER_COST,
            ),
            (
                ZkSyncOp::from(transfer_to_new_op.clone()),
                CommitCost::TRANSFER_TO_NEW_COST,
            ),
            (ZkSyncOp::from(noop_op.clone()), 0),
            (
                ZkSyncOp::from(full_exit_op.clone()),
                CommitCost::FULL_EXIT_COST,
            ),
            (
                ZkSyncOp::from(forced_exit_op.clone()),
                CommitCost::FORCED_EXIT_COST,
            ),
            (
                ZkSyncOp::from(withdraw_op.clone()),
                CommitCost::WITHDRAW_COST,
            ),
            (
                ZkSyncOp::from(withdraw_op.clone()),
                CommitCost::WITHDRAW_COST,
            ),
            (
                ZkSyncOp::from(withdraw_nft_op.clone()),
                CommitCost::WITHDRAW_NFT_COST,
            ),
            (
                ZkSyncOp::from(mint_nft_op.clone()),
                CommitCost::MINT_TOKEN_COST,
            ),
        ];
        let test_vector_verify = vec![
            (
                ZkSyncOp::from(change_pubkey_op),
                VerifyCost::CHANGE_PUBKEY_COST,
            ),
            (ZkSyncOp::from(deposit_op), VerifyCost::DEPOSIT_COST),
            (ZkSyncOp::from(transfer_op), VerifyCost::TRANSFER_COST),
            (
                ZkSyncOp::from(transfer_to_new_op),
                VerifyCost::TRANSFER_TO_NEW_COST,
            ),
            (ZkSyncOp::from(noop_op), 0),
            (ZkSyncOp::from(full_exit_op), VerifyCost::FULL_EXIT_COST),
            (ZkSyncOp::from(forced_exit_op), VerifyCost::FORCED_EXIT_COST),
            (ZkSyncOp::from(withdraw_op), VerifyCost::WITHDRAW_COST),
            (
                ZkSyncOp::from(withdraw_nft_op),
                VerifyCost::WITHDRAW_NFT_COST,
            ),
            (ZkSyncOp::from(mint_nft_op), VerifyCost::MINT_NFT_COST),
        ];

        for (op, expected_cost) in test_vector_commit {
            assert_eq!(
                CommitCost::op_cost(&op),
                U256::from(expected_cost),
                "{:?}",
                &op
            );
        }
        for (op, expected_cost) in test_vector_verify {
            assert_eq!(VerifyCost::op_cost(&op), U256::from(expected_cost));
        }
    }

    #[test]
    fn gas_counter() {
        let change_pubkey_op = ChangePubKeyOp {
            tx: ChangePubKey::new(
                AccountId(1),
                Default::default(),
                Default::default(),
                TokenId(0),
                Default::default(),
                Default::default(),
                Default::default(),
                None,
                None,
            ),
            account_id: AccountId(1),
        };
        let zksync_op = ZkSyncOp::from(change_pubkey_op);

        let mut gas_counter = GasCounter::new();

        assert_eq!(gas_counter.commit_cost, U256::from(CommitCost::BASE_COST));
        assert_eq!(gas_counter.verify_cost, U256::from(VerifyCost::BASE_COST));

        // Verify cost is 0, thus amount of operations is determined by the commit cost.
        let amount_ops_in_block = (U256::from(TX_GAS_LIMIT)
            - GasCounter::scale_up(gas_counter.commit_cost))
            / GasCounter::scale_up(U256::from(CommitCost::OLD_CHANGE_PUBKEY_COST_OFFCHAIN)); // restore after [ZKS-554]

        let mut batch: Vec<_> = (0..amount_ops_in_block.as_u64())
            .map(|_| zksync_op.clone())
            .collect();
        assert!(GasCounter::batch_fits_into_empty_block(&batch));

        batch.push(zksync_op.clone());
        assert!(!GasCounter::batch_fits_into_empty_block(&batch));

        let slice = &[zksync_op.clone()];
        for _ in 0..amount_ops_in_block.as_u64() {
            assert!(gas_counter.can_include(slice));
            gas_counter
                .add_op(&zksync_op)
                .expect("Gas limit was not reached, but op adding failed");
        }

        // Expected gas limit is (base_cost + n_ops * op_cost) * 1.3
        let expected_commit_limit = (U256::from(CommitCost::BASE_COST)
            + amount_ops_in_block * U256::from(CommitCost::OLD_CHANGE_PUBKEY_COST_OFFCHAIN)) // restore after [ZKS-554]
            * U256::from(130)
            / U256::from(100);
        let expected_verify_limit = (U256::from(VerifyCost::BASE_COST)
            + amount_ops_in_block * U256::from(VerifyCost::CHANGE_PUBKEY_COST))
            * U256::from(130)
            / U256::from(100);
        assert_eq!(gas_counter.commit_gas_limit(), expected_commit_limit);
        assert_eq!(gas_counter.verify_gas_limit(), expected_verify_limit);
        assert!(!gas_counter.can_include(slice));

        // Attempt to add one more operation (it should fail).
        gas_counter
            .add_op(&zksync_op)
            .expect_err("Able to add operation beyond the gas limit");

        // Check again that limit has not changed.
        assert_eq!(gas_counter.commit_gas_limit(), expected_commit_limit);
        assert_eq!(gas_counter.verify_gas_limit(), expected_verify_limit);
    }
}
