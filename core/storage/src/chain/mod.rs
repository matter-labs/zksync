pub mod account;
pub mod block;
pub mod operations;
pub mod operations_ext;
pub mod state;
pub mod stats;

use super::StorageProcessor;

/// `ChainIntermediator` is a structure providing methods to
/// obtain schemas declared in the `chain` module.
#[derive(Debug)]
pub struct ChainIntermediator<'a>(pub &'a StorageProcessor);

impl<'a> ChainIntermediator<'a> {
    pub fn account_schema(self) -> account::AccountSchema<'a> {
        account::AccountSchema(self.0)
    }

    pub fn block_schema(self) -> block::BlockSchema<'a> {
        block::BlockSchema(self.0)
    }

    pub fn operations_schema(self) -> operations::OperationsSchema<'a> {
        operations::OperationsSchema(self.0)
    }

    pub fn operations_ext_schema(self) -> operations_ext::OperationsExtSchema<'a> {
        operations_ext::OperationsExtSchema(self.0)
    }

    pub fn state_schema(self) -> state::StateSchema<'a> {
        state::StateSchema(self.0)
    }

    pub fn stats_schema(self) -> stats::StatsSchema<'a> {
        stats::StatsSchema(self.0)
    }
}
