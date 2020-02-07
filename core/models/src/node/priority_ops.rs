use super::AccountId;
use super::TokenId;
use crate::params::{
    ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH, FR_ADDRESS_LEN, TOKEN_BIT_WIDTH,
};
use crate::primitives::{bytes_slice_to_uint32, u128_to_bigdecimal};
use bigdecimal::BigDecimal;
use ethabi::{decode, ParamType};
use failure::{bail, ensure, format_err};
use std::convert::{TryFrom, TryInto};
use std::str::FromStr;
use web3::types::{Address, Log, U256};

use super::operations::{DepositOp, FullExitOp};
use crate::node::operations::ChangePubkeyOnchainOp;
use crate::node::PubKeyHash;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    pub from: Address,
    pub token: TokenId,
    pub amount: BigDecimal,
    pub to: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullExit {
    pub account_id: AccountId,
    pub eth_address: Address,
    pub token: TokenId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangePubKeyOnchain {
    pub new_pubkey_hash: PubKeyHash,
    pub eth_address: Address,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinPriorityOp {
    Deposit(Deposit),
    FullExit(FullExit),
    ChangePubKeyOnchain(ChangePubKeyOnchain),
}

impl FranklinPriorityOp {
    pub fn parse_from_priority_queue_logs(
        pub_data: &[u8],
        op_type_id: u8,
    ) -> Result<Self, failure::Error> {
        match op_type_id {
            DepositOp::OP_CODE => {
                let (sender, pub_data_left) = {
                    let (sender, left) = pub_data.split_at(ETH_ADDRESS_BIT_WIDTH / 8);
                    (Address::from_slice(sender), left)
                };
                let (token, pub_data_left) = {
                    let (token, left) = pub_data_left.split_at(TOKEN_BIT_WIDTH / 8);
                    (u16::from_be_bytes(token.try_into().unwrap()), left)
                };
                let (amount, pub_data_left) = {
                    let (amount, left) = pub_data_left.split_at(BALANCE_BIT_WIDTH / 8);
                    let amount = u128::from_be_bytes(amount.try_into().unwrap());
                    (u128_to_bigdecimal(amount), left)
                };
                let (account, pub_data_left) = {
                    let (account, left) = pub_data_left.split_at(FR_ADDRESS_LEN);
                    (Address::from_slice(account), left)
                };
                ensure!(
                    pub_data_left.is_empty(),
                    "DepositOp parse failed: input too big"
                );
                Ok(Self::Deposit(Deposit {
                    from: sender,
                    token,
                    amount,
                    to: account,
                }))
            }
            FullExitOp::OP_CODE => {
                let (account_id, pub_data_left) = {
                    let (account_id, left) = pub_data.split_at(ACCOUNT_ID_BIT_WIDTH / 8);
                    (bytes_slice_to_uint32(account_id).unwrap(), left)
                };
                let (eth_address, pub_data_left) = {
                    let (eth_address, left) = pub_data_left.split_at(ETH_ADDRESS_BIT_WIDTH / 8);
                    (Address::from_slice(eth_address), left)
                };
                let (token, pub_data_left) = {
                    let (token, left) = pub_data_left.split_at(TOKEN_BIT_WIDTH / 8);
                    (u16::from_be_bytes(token.try_into().unwrap()), left)
                };
                ensure!(
                    pub_data_left.is_empty(),
                    "FullExitOp parse failed: input too big"
                );
                Ok(Self::FullExit(FullExit {
                    account_id,
                    eth_address,
                    token,
                }))
            }
            ChangePubkeyOnchainOp::OP_CODE => {
                let (new_pubkey_hash, pub_data_left) = {
                    let (pubkey_hash, left) = pub_data.split_at(ETH_ADDRESS_BIT_WIDTH / 8);
                    (PubKeyHash::from_bytes(pubkey_hash)?, left)
                };
                let (eth_address, pub_data_left) = {
                    let (eth_address, left) = pub_data_left.split_at(ETH_ADDRESS_BIT_WIDTH / 8);
                    (Address::from_slice(eth_address), left)
                };
                ensure!(
                    pub_data_left.is_empty(),
                    "ChangePubkeyPriorityOp parse failed: input too big"
                );
                Ok(Self::ChangePubKeyOnchain(ChangePubKeyOnchain {
                    new_pubkey_hash,
                    eth_address,
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
            Self::ChangePubKeyOnchain(_) => ChangePubkeyOnchainOp::CHUNKS,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityOp {
    pub serial_id: u64,
    pub data: FranklinPriorityOp,
    pub deadline_block: u64,
    pub eth_fee: BigDecimal,
    pub eth_hash: Vec<u8>,
}

impl TryFrom<Log> for PriorityOp {
    type Error = failure::Error;

    fn try_from(event: Log) -> Result<PriorityOp, failure::Error> {
        let mut dec_ev = decode(
            &[
                ParamType::Uint(64),  // Serial id
                ParamType::Uint(8),   // OpType
                ParamType::Bytes,     // Pubdata
                ParamType::Uint(256), // expir. block
                ParamType::Uint(256), // fee
            ],
            &event.data.0,
        )
        .map_err(|e| format_err!("Event data decode: {:?}", e))?;

        Ok(PriorityOp {
            serial_id: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u64)
                .unwrap(),
            data: {
                let op_type = dec_ev
                    .remove(0)
                    .to_uint()
                    .as_ref()
                    .map(|ui| U256::as_u32(ui) as u8)
                    .unwrap();
                let op_pubdata = dec_ev.remove(0).to_bytes().unwrap();
                FranklinPriorityOp::parse_from_priority_queue_logs(&op_pubdata, op_type)
                    .expect("Failed to parse priority op data")
            },
            deadline_block: dec_ev
                .remove(0)
                .to_uint()
                .as_ref()
                .map(U256::as_u64)
                .unwrap(),
            eth_fee: {
                let amount_uint = dec_ev.remove(0).to_uint().unwrap();
                BigDecimal::from_str(&format!("{}", amount_uint)).unwrap()
            },
            eth_hash: event
                .transaction_hash
                .expect("Event transaction hash is missing")
                .as_bytes()
                .to_vec(),
        })
    }
}
