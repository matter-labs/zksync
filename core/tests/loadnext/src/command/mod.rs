use rand::{thread_rng, Rng};
use zksync_types::Address;

use crate::account_pool::AddressPool;

pub use self::{
    api_command::ApiRequestCommand,
    tx_command::{ExpectedOutcome, IncorrectnessModifier, TxCommand, TxType},
};

mod api_command;
mod tx_command;

#[derive(Debug, Clone)]
pub enum Command {
    SingleTx(TxCommand),
    Batch(Vec<TxCommand>),
    ApiRequest(ApiRequestCommand),
}

impl Command {
    pub const MAX_BATCH_SIZE: usize = 20;

    pub fn random(own_address: Address, addresses: &AddressPool) -> Self {
        let rng = &mut thread_rng();
        let chance = rng.gen_range(0.0f32, 1.0f32);

        // We have a 70% tx command rate amd 30% batch command rate.
        // We don't generate API requests at the moment.
        if chance < 0.7 {
            Self::SingleTx(TxCommand::random(own_address, addresses))
        } else {
            let batch_size = rng.gen_range(1, Self::MAX_BATCH_SIZE + 1);
            let batch_command = (0..batch_size)
                .map(|_| TxCommand::random_batchable(own_address, addresses))
                .collect();
            Self::Batch(batch_command)
        }
    }
}
