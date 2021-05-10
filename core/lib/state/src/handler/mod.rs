use crate::state::{CollectedFee, OpSuccess};
use zksync_types::AccountUpdates;

#[macro_use]
mod invariant;
mod change_pubkey;
mod close;
mod deposit;
pub mod error;
mod forced_exit;
mod full_exit;
mod mint_nft;
mod swap;
mod transfer;
mod withdraw;
mod withdraw_nft;

/// TxHandler trait encapsulates the logic of each individual transaction
/// handling. By transactions we assume both zkSync network transactions,
/// and priority operations (initiated by invoking the Ethereum smart contract
/// methods).
///
/// Template parameter `Tx` represents a type of transaction being handled.
/// It has to be a template parameter rather than an associated type, so
/// there may be more than one trait implementation for a structure.
pub trait TxHandler<Tx> {
    /// Operation wrapper for the transaction.
    type Op;

    type OpError;

    /// Creates an operation wrapper from the given transaction.
    fn create_op(&self, tx: Tx) -> Result<Self::Op, Self::OpError>;

    /// Applies the transaction.
    fn apply_tx(&mut self, tx: Tx) -> Result<OpSuccess, Self::OpError>;

    /// Applies the operation.
    fn apply_op(
        &mut self,
        op: &Self::Op,
    ) -> Result<(Option<CollectedFee>, AccountUpdates), Self::OpError>;
}
