use super::operations::{DepositOp, FullExitOp};
use super::{AccountAddress, AccountId, TokenId};
use crate::params::FR_ADDRESS_LEN;
use bigdecimal::BigDecimal;
use failure::{bail, ensure};
use std::convert::TryInto;
use web3::types::Address;

// From enum OpType in Franklin.sol
const DEPOSIT_OPTYPE_ID: u8 = 1u8;
const FULLEXIT_OPTYPE_ID: u8 = 6u8;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    pub sender: Address,
    pub token: TokenId,
    pub amount: BigDecimal,
    pub account: AccountAddress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullExit {
    pub account_id: AccountId,
    pub eth_address: Address,
    pub token: TokenId,
    pub signature_r: Box<[u8; 32]>,
    pub signature_s: Box<[u8; 32]>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FranklinPriorityOp {
    Deposit(Deposit),
    FullExit(FullExit),
}

impl FranklinPriorityOp {
    pub fn parse_pubdata(pub_data: &[u8], op_type_id: u8) -> Result<Self, failure::Error> {
        match op_type_id {
            DEPOSIT_OPTYPE_ID => {
                ensure!(
                    pub_data.len() == 20 + 2 + 16 + FR_ADDRESS_LEN,
                    "Pub data len mismatch"
                );
                let sender = Address::from_slice(&pub_data[0..20]);
                let token = u16::from_be_bytes(pub_data[20..(20 + 2)].try_into().unwrap());
                let amount = {
                    let amount = u128::from_be_bytes(pub_data[22..(22 + 16)].try_into().unwrap());
                    amount.to_string().parse().unwrap()
                };
                let account =
                    AccountAddress::from_bytes(&pub_data[38..(38 + FR_ADDRESS_LEN)]).unwrap();
                Ok(Self::Deposit(Deposit {
                    sender,
                    token,
                    amount,
                    account,
                }))
            }
            FULLEXIT_OPTYPE_ID => {
                ensure!(pub_data.len() == 3 + 20 + 2 + 64, "Pub data len mismatch");
                let account_id = {
                    let mut account_id_bytes = [0u8; 4];
                    account_id_bytes[1..4].copy_from_slice(&pub_data[0..3]);
                    u32::from_be_bytes(account_id_bytes)
                };
                let eth_address = Address::from_slice(&pub_data[2..(20 + 2)]);
                let token = u16::from_be_bytes(pub_data[22..(22 + 2)].try_into().unwrap());
                let signature_r = Box::new(pub_data[24..(24 + 32)].try_into().unwrap());
                let signature_s = Box::new(pub_data[56..(56 + 32)].try_into().unwrap());
                Ok(Self::FullExit(FullExit {
                    account_id,
                    eth_address,
                    token,
                    signature_r,
                    signature_s,
                }))
            }
            _ => {
                bail!("Unsupported priority op type");
            }
        }
    }

    pub fn chunks(&self) -> usize {
        match self {
            Self::Deposit(_) => DepositOp::CHUNKS,
            Self::FullExit(_) => FullExitOp::CHUNKS,
        }
    }
}
