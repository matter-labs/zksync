pub mod account;
pub mod block;
pub mod mempool;
pub mod operations;
pub mod operations_ext;
pub mod state;
pub mod stats;

use super::StorageProcessor;

/// `ChainIntermediator` is a structure providing methods to
/// obtain schemas declared in the `chain` module.
pub struct ChainIntermediator<'a>(pub &'a StorageProcessor);

impl<'a> ChainIntermediator<'a> {
    pub fn account(self) -> account::AccountSchema<'a> {
        account::AccountSchema(self.0)
    }

    pub fn block(self) -> block::BlockSchema<'a> {
        block::BlockSchema(self.0)
    }

    pub fn operations(self) -> operations::OperationsSchema<'a> {
        operations::OperationsSchema(self.0)
    }

    pub fn operations_ext(self) -> operations_ext::OperationsExtSchema<'a> {
        operations_ext::OperationsExtSchema(self.0)
    }

    pub fn state(self) -> state::StateSchema<'a> {
        state::StateSchema(self.0)
    }

    pub fn stats(self) -> stats::StatsSchema<'a> {
        stats::StatsSchema(self.0)
    }
}
