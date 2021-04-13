use std::convert::TryFrom;

use ethabi::{decode, ParamType};

use zksync_basic_types::Log;

use crate::{AccountId, Address, BlockNumber};

#[derive(Clone, Debug)]
pub struct RegisterNFTFactoryEvent {
    pub factory_address: Address,
    pub creator_address: Address,
    pub creator_signature: Vec<u8>,
    pub eth_block: u64,
}

impl TryFrom<Log> for RegisterNFTFactoryEvent {
    type Error = anyhow::Error;

    fn try_from(event: Log) -> Result<Self, anyhow::Error> {
        let eth_block = match event.block_number {
            Some(block_number) => block_number.as_u64(),
            None => {
                return Err(anyhow::format_err!(
                    "Failed to parse RegisterNFTFactoryEvent: {:#?}",
                    event
                ))
            }
        };

        let mut decoded_event = decode(
            &[
                ParamType::Address, // factoryAddress
                ParamType::Bytes,   // signature
            ],
            &event.data.0,
        )
        .map_err(|e| anyhow::format_err!("Event data decode: {:?}", e))?;
        let creator_address = Address::from_slice(&event.topics[1].as_fixed_bytes()[12..]);
        let factory_address = decoded_event.remove(0).to_address().unwrap();
        let signature = decoded_event.remove(0).to_bytes().unwrap();
        Ok(Self {
            factory_address,
            creator_address,
            creator_signature: signature,
            eth_block,
        })
    }
}
