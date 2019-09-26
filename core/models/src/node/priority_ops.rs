use super::operations::{DepositOp, FullExitOp};
use super::tx::{PackedPublicKey, PackedSignature, TxSignature};
use super::Nonce;
use super::{AccountAddress, TokenId};
use crate::params::FR_ADDRESS_LEN;
use bigdecimal::BigDecimal;
use failure::{bail, ensure};
use std::convert::TryInto;
use web3::types::Address;

use super::operations::{
    DEPOSIT_OP_CODE,
    TRANSFER_TO_NEW_OP_CODE,
    WITHDRAW_OP_CODE,
    CLOSE_OP_CODE,
    TRANSFER_OP_CODE,
    FULL_EXIT_OP_CODE,
    TX_TYPE_BYTES_LEGTH,
    ACCOUNT_ID_BYTES_LEGTH,
    TOKEN_BYTES_LENGTH,
    FULL_AMOUNT_BYTES_LEGTH,
    FEE_BYTES_LEGTH,
    ETH_ADDR_BYTES_LEGTH,
    PACKED_AMOUNT_BYTES_LEGTH,
    NONCE_BYTES_LEGTH,
    SIGNATURE_R_BYTES_LEGTH,
    SIGNATURE_S_BYTES_LEGTH,
    PUBKEY_PACKED_BYTES_LEGTH,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Deposit {
    pub sender: Address,
    pub token: TokenId,
    pub amount: BigDecimal,
    pub account: AccountAddress,
}

impl Deposit {
    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let token_id_pre_length = ACCOUNT_ID_BYTES_LEGTH;
        let amount_pre_length = token_id_pre_length +
            TOKEN_BYTES_LENGTH;
        let account_pre_length = amount_pre_length +
            FULL_AMOUNT_BYTES_LEGTH +
            FEE_BYTES_LEGTH;
        Self {
            sender: Address::zero(), // In current circuit there is no sender in deposit pubdata
            token: TokenId::from_be_bytes(bytes[token_id_pre_length .. token_id_pre_length + TOKEN_BYTES_LENGTH]),
            amount: BigDecimal::parse_bytes(bytes[amount_pre_length .. amount_pre_length + FULL_AMOUNT_BYTES_LEGTH].to_vec(), 18),
            account: AccountAddress::from_bytes(bytes[account_pre_length .. account_pre_length + FR_ADDRESS_LEN]),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullExit {
    pub packed_pubkey: Box<[u8; PUBKEY_PACKED_BYTES_LEGTH]>,
    pub eth_address: Address,
    pub token: TokenId,
    pub nonce: Nonce,
    pub signature_r: Box<[u8; SIGNATURE_R_BYTES_LEGTH]>,
    pub signature_s: Box<[u8; SIGNATURE_S_BYTES_LEGTH]>,
}

impl FullExit {
    const TX_TYPE: u8 = 6;

    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let packed_pubkey_pre_length = ACCOUNT_ID_BYTES_LEGTH;
        let eth_address_pre_length = packed_pubkey_pre_length +
            PUBKEY_PACKED_BYTES_LEGTH;
        let token_pre_length = eth_address_pre_length +
            ETH_ADDR_BYTES_LEGTH;
        let nonce_pre_length = token_pre_length +
            TOKEN_BYTES_LENGTH;
        let signature_r_pre_length = nonce_pre_length +
            NONCE_BYTES_LEGTH;
        let signature_s_pre_length = signature_r_pre_length +
            SIGNATURE_R_BYTES_LEGTH;
        Self {
            packed_pubkey: Box::from(bytes[packed_pubkey_pre_length .. packed_pubkey_pre_length + PUBKEY_PACKED_BYTES_LEGTH]),
            eth_address: Address::from_slice(bytes[eth_address_pre_length .. eth_address_pre_length + ETH_ADDR_BYTES_LEGTH]),
            token: TokenId::from_be_bytes(bytes[token_pre_length .. token_pre_length + TOKEN_BYTES_LENGTH]),
            nonce: Nonce::from_be_bytes(bytes[nonce_pre_length .. nonce_pre_length + NONCE_BYTES_LEGTH]),
            signature_r: Box::from(bytes[signature_r_pre_length .. signature_r_pre_length + SIGNATURE_R_BYTES_LEGTH]),
            signature_s: Box::from(bytes[signature_s_pre_length .. signature_s_pre_length + SIGNATURE_S_BYTES_LEGTH])
        }
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(self.packed_pubkey.as_ref());
        out.extend_from_slice(&self.eth_address.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    pub fn verify_signature(&self) -> Option<AccountAddress> {
        let mut sign = Vec::with_capacity(64);
        sign.extend_from_slice(self.signature_r.as_ref());
        sign.extend_from_slice(self.signature_s.as_ref());

        let sign = if let Ok(sign) = PackedSignature::deserialize_packed(&sign) {
            sign
        } else {
            return None;
        };

        let pub_key =
            if let Ok(pub_key) = PackedPublicKey::deserialize_packed(self.packed_pubkey.as_ref()) {
                pub_key
            } else {
                return None;
            };

        let restored_signature = TxSignature { pub_key, sign };

        restored_signature
            .verify_musig_pedersen(&self.get_bytes())
            .map(AccountAddress::from_pubkey)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FranklinPriorityOp {
    Deposit(Deposit),
    FullExit(FullExit),
}

impl FranklinPriorityOp {
    pub fn parse_pubdata(pub_data: &[u8], op_type_id: u8) -> Result<Self, failure::Error> {
        match op_type_id {
            DEPOSIT_OP_CODE => {
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
            FULL_EXIT_OP_CODE => {
                ensure!(
                    pub_data.len() == 32 + 20 + 2 + 64 + 4,
                    "Pub data len mismatch"
                );
                let packed_pubkey = Box::new(pub_data[0..32].try_into().unwrap());
                let eth_address = Address::from_slice(&pub_data[32..(32 + 20)]);
                let token = u16::from_be_bytes(pub_data[52..(52 + 2)].try_into().unwrap());
                let nonce = u32::from_be_bytes(pub_data[54..(54 + 4)].try_into().unwrap());
                let signature_r = Box::new(pub_data[58..(58 + 32)].try_into().unwrap());
                let signature_s = Box::new(pub_data[90..(90 + 32)].try_into().unwrap());
                Ok(Self::FullExit(FullExit {
                    packed_pubkey,
                    eth_address,
                    token,
                    nonce,
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
