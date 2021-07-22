use zksync_types::{
    gas_counter::{CommitCost, VerifyCost},
    ChangePubKeyOp, MintNFTOp, SwapOp, TransferOp, TransferToNewOp, WithdrawNFTOp, WithdrawOp,
};

/// Gas cost per chunk to cover constant cost of commit, execute and prove transactions
pub(crate) const AMORTIZED_COST_PER_CHUNK: u64 = 200;
// Base operation costs estimated via `gas_price` test.
// Factor of AMORTIZED_COST_PER_CHUNK * CHUNKS accounts for constant overhead of the commit, execute, prove for blocks of 680 chunks
// where we assume that we commit 5 blocks at once, prove 10 and execute 5
pub(crate) const BASE_TRANSFER_COST: u64 = VerifyCost::TRANSFER_COST
    + CommitCost::TRANSFER_COST
    + AMORTIZED_COST_PER_CHUNK * (TransferOp::CHUNKS as u64);
pub(crate) const BASE_TRANSFER_TO_NEW_COST: u64 = VerifyCost::TRANSFER_TO_NEW_COST
    + CommitCost::TRANSFER_TO_NEW_COST
    + AMORTIZED_COST_PER_CHUNK * (TransferToNewOp::CHUNKS as u64);
pub(crate) const BASE_WITHDRAW_COST: u64 = VerifyCost::WITHDRAW_COST
    + CommitCost::WITHDRAW_COST
    + AMORTIZED_COST_PER_CHUNK * (WithdrawOp::CHUNKS as u64);
pub(crate) const BASE_WITHDRAW_NFT_COST: u64 = VerifyCost::WITHDRAW_NFT_COST
    + CommitCost::WITHDRAW_NFT_COST
    + AMORTIZED_COST_PER_CHUNK * (WithdrawNFTOp::CHUNKS as u64);
pub(crate) const BASE_OLD_CHANGE_PUBKEY_OFFCHAIN_COST: u64 =
    CommitCost::OLD_CHANGE_PUBKEY_COST_OFFCHAIN
        + VerifyCost::CHANGE_PUBKEY_COST
        + AMORTIZED_COST_PER_CHUNK * (ChangePubKeyOp::CHUNKS as u64);
pub(crate) const BASE_CHANGE_PUBKEY_OFFCHAIN_COST: u64 = CommitCost::CHANGE_PUBKEY_COST_OFFCHAIN
    + VerifyCost::CHANGE_PUBKEY_COST
    + AMORTIZED_COST_PER_CHUNK * (ChangePubKeyOp::CHUNKS as u64);
pub(crate) const BASE_CHANGE_PUBKEY_CREATE2_COST: u64 = CommitCost::CHANGE_PUBKEY_COST_CREATE2
    + VerifyCost::CHANGE_PUBKEY_COST
    + AMORTIZED_COST_PER_CHUNK * (ChangePubKeyOp::CHUNKS as u64);
pub(crate) const BASE_CHANGE_PUBKEY_ONCHAIN_COST: u64 = CommitCost::CHANGE_PUBKEY_COST_ONCHAIN
    + VerifyCost::CHANGE_PUBKEY_COST
    + AMORTIZED_COST_PER_CHUNK * (ChangePubKeyOp::CHUNKS as u64);
pub(crate) const BASE_MINT_NFT_COST: u64 = VerifyCost::MINT_NFT_COST
    + CommitCost::MINT_TOKEN_COST
    + AMORTIZED_COST_PER_CHUNK * (MintNFTOp::CHUNKS as u64);
pub(crate) const BASE_SWAP_COST: u64 = CommitCost::SWAP_COST
    + VerifyCost::SWAP_COST
    + AMORTIZED_COST_PER_CHUNK * (SwapOp::CHUNKS as u64);
