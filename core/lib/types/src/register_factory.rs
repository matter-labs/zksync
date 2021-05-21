use std::convert::TryFrom;

use ethabi::{decode, ParamType};
use thiserror::Error;

use zksync_basic_types::Log;

use crate::Address;

#[derive(Debug, Error)]
pub enum RegisterNFTFactoryEventParseError {
    #[error("Cannot parse log for Register Factory Event {0:?}")]
    ParseLogError(Log),
    #[error("Cannot parse log for Register Factory Event {0:?}")]
    ParseError(ethabi::Error),
}

#[derive(Clone, Debug)]
pub struct RegisterNFTFactoryEvent {
    pub factory_address: Address,
    pub creator_address: Address,
    pub eth_block: u64,
}

impl TryFrom<Log> for RegisterNFTFactoryEvent {
    type Error = RegisterNFTFactoryEventParseError;

    fn try_from(event: Log) -> Result<Self, Self::Error> {
        let eth_block = match event.block_number {
            Some(block_number) => block_number.as_u64(),
            None => return Err(RegisterNFTFactoryEventParseError::ParseLogError(event)),
        };

        let mut decoded_event = decode(
            &[
                ParamType::Address, // factoryAddress
            ],
            &event.data.0,
        )
        .map_err(RegisterNFTFactoryEventParseError::ParseError)?;
        let creator_address = Address::from_slice(&event.topics[2].as_fixed_bytes()[12..]);
        let factory_address = decoded_event.remove(0).to_address().unwrap();
        Ok(Self {
            factory_address,
            creator_address,
            eth_block,
        })
    }
}
