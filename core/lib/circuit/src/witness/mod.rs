use zksync_crypto::franklin_crypto::bellman::pairing::bn256::Bn256;

use zksync_crypto::circuit::account::CircuitAccountTree;

use crate::operation::Operation;

// Public re-exports
pub use self::{
    change_pubkey_offchain::ChangePubkeyOffChainWitness,
    close_account::CloseAccountWitness,
    deposit::DepositWitness,
    forced_exit::ForcedExitWitness,
    full_exit::FullExitWitness,
    transfer::TransferWitness,
    transfer_to_new::TransferToNewWitness,
    utils::{SigDataInput, WitnessBuilder},
    withdraw::WithdrawWitness,
};

pub mod change_pubkey_offchain;
pub mod close_account;
pub mod deposit;
pub mod forced_exit;
pub mod full_exit;
pub mod noop;
pub mod transfer;
pub mod transfer_to_new;
pub mod withdraw;

pub mod utils;

#[cfg(test)]
pub(crate) mod tests;

/// Generic trait representing the witness data interface.
pub trait Witness {
    /// Type of the operation generating the witness.
    type OperationType;
    /// Additional data required for calculating the Circuit operations.
    /// Should be `()` if no additional data required.
    type CalculateOpsInput;

    /// Applies the operation to the Circuit account tree, generating the witness data.
    fn apply_tx(tree: &mut CircuitAccountTree, op: &Self::OperationType) -> Self;

    /// Obtains the pubdata from the witness.
    fn get_pubdata(&self) -> Vec<bool>;

    /// Calculates the list of Circuit operations from the witness data.
    fn calculate_operations(&self, input: Self::CalculateOpsInput) -> Vec<Operation<Bn256>>;
}
