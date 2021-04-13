use std::convert::TryFrom;
use zksync_basic_types::Log;

use crate::tx::PackedEthSignature;
use crate::{AccountId, Address, BlockNumber};

#[derive(Clone, Debug)]
pub struct RegisterNFTFactoryEvent {
    pub factory_address: Address,
    pub creator_address: Address,
    pub creator_signature: PackedEthSignature,
    pub eth_block: u64,
}

impl TryFrom<Log> for RegisterNFTFactoryEvent {
    type Error = anyhow::Error;

    fn try_from(event: Log) -> Result<Self, anyhow::Error> {
        let eth_block_number = match event.block_number {
            Some(block_number) => block_number.as_u64(),
            None => {
                return Err(anyhow::format_err!(
                    "Failed to parse RegisterNFTFactoryEvent: {:#?}",
                    event
                ))
            }
        };
        todo!()

        // Ok(NewTokenEvent {
        //     eth_block_number,
        //     address: Address::from_slice(&event.topics[1].as_fixed_bytes()[12..]),
        //     id: TokenId(U256::from_big_endian(&event.topics[2].as_fixed_bytes()[..]).as_u32()),
        // })
    }
}
