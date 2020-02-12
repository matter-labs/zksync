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
#[serde(tag = "type")]
pub enum FranklinPriorityOp {
    Deposit(Deposit),
    FullExit(FullExit),
}

impl FranklinPriorityOp {
    pub fn parse_from_priority_queue_logs(
        pub_data: &[u8],
        op_type_id: u8,
    ) -> Result<Self, failure::Error> {

        // see contracts/contracts/Operations.sol
        match op_type_id {
            DepositOp::OP_CODE => {
                let pub_data_left = pub_data;

                // account_id
                let (_, pub_data_left) = pub_data_left.split_at(ACCOUNT_ID_BIT_WIDTH / 8);

                // token
                let (token, pub_data_left) = {
                    let (token, left) = pub_data_left.split_at(TOKEN_BIT_WIDTH / 8);
                    (u16::from_be_bytes(token.try_into().unwrap()), left)
                };

                // amount
                let (amount, pub_data_left) = {
                    let (amount, left) = pub_data_left.split_at(BALANCE_BIT_WIDTH / 8);
                    let amount = u128::from_be_bytes(amount.try_into().unwrap());
                    (u128_to_bigdecimal(amount), left)
                };

                // pubkey_hash
                let (account, pub_data_left) = {
                    let (account, left) = pub_data_left.split_at(FR_ADDRESS_LEN);
                    (Address::from_slice(account), left)
                };

                ensure!(
                    pub_data_left.len() == 0,
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

                // account_id
                let (account_id, pub_data_left) = {
                    let (account_id, left) = pub_data.split_at(ACCOUNT_ID_BIT_WIDTH / 8);
                    (bytes_slice_to_uint32(account_id).unwrap(), left)
                };

                // pubkey -- TODO: rename??!
                let (packed_pubkey, pub_data_left) = {
                    let (packed_pubkey, left) =
                        pub_data_left.split_at(SUBTREE_HASH_WIDTH_PADDED / 8);
                    (Box::new(packed_pubkey.try_into().unwrap()), left)
                };

                // owner
                let (eth_address, pub_data_left) = {
                    let (eth_address, left) = pub_data_left.split_at(ETH_ADDRESS_BIT_WIDTH / 8);
                    (Address::from_slice(eth_address), left)
                };

                // token
                let (token, pub_data_left) = {
                    let (token, left) = pub_data_left.split_at(TOKEN_BIT_WIDTH / 8);
                    (u16::from_be_bytes(token.try_into().unwrap()), left)
                };

                // nonce
                let (nonce, pub_data_left) = {
                    let (nonce, left) = pub_data_left.split_at(NONCE_BIT_WIDTH / 8);
                    (u32::from_be_bytes(nonce.try_into().unwrap()), left)
                };

                // sig_r
                let (signature_r, pub_data_left) = {
                    let (signature_r, left) =
                        pub_data_left.split_at(SIGNATURE_R_BIT_WIDTH_PADDED / 8);
                    (Box::new(signature_r.try_into().unwrap()), left)
                };

                // sig_s
                let (signature_s, pub_data_left) = {
                    let (signature_s, left) =
                        pub_data_left.split_at(SIGNATURE_S_BIT_WIDTH_PADDED / 8);
                    (Box::new(signature_s.try_into().unwrap()), left)
                };

                // amount
                ensure!(
                    pub_data_left.len() == BALANCE_BIT_WIDTH / 8,
                    "FullExitOp parse failed: input too big"
                );

                Ok(Self::FullExit(FullExit {
                    account_id,
                    eth_address,
                    token,
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
