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
            // TODO: For some reason, batches of size 1 are being rejected because of nonce mistmatch.
            // It may be either bug in loadtest or server code, thus it should be investigated.
            let batch_size = rng.gen_range(2, Self::MAX_BATCH_SIZE + 1);
            let mut batch_command: Vec<_> = (0..batch_size)
                .map(|_| TxCommand::random_batchable(own_address, addresses))
                .collect();

            if batch_command
                .iter()
                .any(|cmd| cmd.modifier == IncorrectnessModifier::ZeroFee)
            {
                // Zero fee modifier is kinda weird for batches, since the summary fee may be enough to cover
                // cost of one tx with zero fee. Thus in that case we set zero fee modifier to all the transactions.
                // Note that behavior in the statement above is not a bug: to live in the volatile world of Ethereum,
                // server may accept batches with the fee slightly below that what has been reported to user via API.
                for command in batch_command.iter_mut() {
                    command.modifier = IncorrectnessModifier::ZeroFee;
                }
            }

            Self::Batch(batch_command)
        }
    }
}
