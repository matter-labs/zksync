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
    pub fn random(own_address: Address, addresses: &AddressPool) -> Self {
        const MAX_BATCH_SIZE: usize = 20;

        let rng = &mut thread_rng();
        let chance = rng.gen_range(0.0f32, 1.0f32);

        // We have a 40% tx command rate, 10% batch command rate, and 50% API command rate.
        if chance < 0.4 {
            Self::SingleTx(TxCommand::random(own_address, addresses))
        } else if chance < 0.5 {
            let batch_size = rng.gen_range(1, MAX_BATCH_SIZE + 1);
            let batch_command = (0..batch_size)
                .map(|_| TxCommand::random_batchable(own_address, addresses))
                .collect();
            Self::Batch(batch_command)
        } else {
            Self::ApiRequest(ApiRequestCommand::random(own_address, addresses))
        }
    }
}
