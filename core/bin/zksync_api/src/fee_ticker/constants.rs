use zksync_types::{
    config::MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL,
    gas_counter::{CommitCost, GasCounter, VerifyCost},
    ChangePubKeyOp, TransferOp, TransferToNewOp, WithdrawOp,
};

// Base operation costs estimated via `gas_price` test.
//
// Factor of 1000 * CHUNKS accounts for constant overhead of the commit and verify for block of 680 chunks
// (140k + 530k) / 680. Should be removed after recursion is introduced to mainnet.
pub(crate) const BASE_TRANSFER_COST: u64 =
    VerifyCost::TRANSFER_COST + CommitCost::TRANSFER_COST + 1000 * (TransferOp::CHUNKS as u64);
pub(crate) const BASE_TRANSFER_TO_NEW_COST: u64 = VerifyCost::TRANSFER_TO_NEW_COST
    + CommitCost::TRANSFER_TO_NEW_COST
    + 1000 * (TransferToNewOp::CHUNKS as u64);
pub(crate) const BASE_WITHDRAW_COST: u64 = VerifyCost::WITHDRAW_COST
    + CommitCost::WITHDRAW_COST
    + GasCounter::COMPLETE_WITHDRAWALS_COST
    + 1000 * (WithdrawOp::CHUNKS as u64)
    + (GasCounter::COMPLETE_WITHDRAWALS_BASE_COST / MAX_WITHDRAWALS_TO_COMPLETE_IN_A_CALL);
pub(crate) const BASE_CHANGE_PUBKEY_OFFCHAIN_COST: u64 = CommitCost::CHANGE_PUBKEY_COST_OFFCHAIN
    + VerifyCost::CHANGE_PUBKEY_COST
    + 1000 * (ChangePubKeyOp::CHUNKS as u64);
pub(crate) const BASE_CHANGE_PUBKEY_ONCHAIN_COST: u64 = CommitCost::CHANGE_PUBKEY_COST_ONCHAIN
    + zksync_types::gas_counter::VerifyCost::CHANGE_PUBKEY_COST
    + 1000 * (ChangePubKeyOp::CHUNKS as u64);

// The Subsidized cost of operations.
// Represent the cost of performing operations after recursion is introduced to mainnet.
pub(crate) const SUBSIDY_TRANSFER_COST: u64 = 550;
pub(crate) const SUBSIDY_TRANSFER_TO_NEW_COST: u64 = 550 * 3;
pub(crate) const SUBSIDY_WITHDRAW_COST: u64 = 45000;
pub(crate) const SUBSIDY_CHANGE_PUBKEY_OFFCHAIN_COST: u64 = 10000;
