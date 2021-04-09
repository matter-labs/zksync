use rand::Rng;
use static_assertions::const_assert;

use zksync_types::Address;

use crate::{
    account_pool::AddressPool,
    constants::MAX_BATCH_SIZE,
    rng::{LoadtestRng, Random},
};

pub use self::{
    api_command::ApiRequestCommand,
    tx_command::{ExpectedOutcome, IncorrectnessModifier, TxCommand, TxType},
};

mod api_command;
mod tx_command;

/// Generic command that can be executed by the loadtest.
///
/// `Command::ApiRequest` is currently unused.
#[derive(Debug, Clone)]
pub enum Command {
    SingleTx(TxCommand),
    Batch(Vec<TxCommand>),
    ApiRequest(ApiRequestCommand),
}

/// Decision regarding the type of command to be spawned.
#[derive(Debug, Clone, Copy)]
enum CommandType {
    SingleTx,
    Batch,
    ApiRequest,
}

impl Random for CommandType {
    fn random(rng: &mut LoadtestRng) -> Self {
        // Chances of a certain event generation.
        // You must maintain the sum of these constants to be equal to 1.0f32.
        const SINGLE_TX_CHANCE: f32 = 0.7;
        const BATCH_CHANCE: f32 = 0.3;
        // We don't generate API requests at the moment.
        const _API_REQUEST_CHANCE: f32 = 0.0;

        const _CHANCES_SUM: f32 = SINGLE_TX_CHANCE + BATCH_CHANCE + _API_REQUEST_CHANCE;
        // Unfortunately. f64::abs()` is not yet a `const` function.
        const_assert!(
            -f32::EPSILON <= (_CHANCES_SUM - 1.0f32) && (_CHANCES_SUM - 1.0f32) <= f32::EPSILON
        );
        let chance = rng.gen_range(0.0f32, 1.0f32);

        if chance <= SINGLE_TX_CHANCE {
            Self::SingleTx
        } else if chance <= (SINGLE_TX_CHANCE + BATCH_CHANCE) {
            Self::Batch
        } else {
            Self::ApiRequest
        }
    }
}

impl Command {
    pub fn random(rng: &mut LoadtestRng, own_address: Address, addresses: &AddressPool) -> Self {
        match CommandType::random(rng) {
            CommandType::SingleTx => Self::SingleTx(TxCommand::random(rng, own_address, addresses)),
            CommandType::Batch => {
                // TODO: For some reason, batches of size 1 are being rejected because of nonce mistmatch.
                // It may be either bug in loadtest or server code, thus it should be investigated.
                let batch_size = rng.gen_range(2, MAX_BATCH_SIZE + 1);
                let mut batch_command: Vec<_> = (0..batch_size)
                    .map(|_| TxCommand::random_batchable(rng, own_address, addresses))
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
            CommandType::ApiRequest => {
                unreachable!("We don't generate API commands currently")
            }
        }
    }
}
