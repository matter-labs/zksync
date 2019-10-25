use super::tx::TxSignature;
use super::AccountAddress;
use super::AccountId;
use super::FranklinTx;
use crate::node::{
    pack_fee_amount, pack_token_amount, unpack_fee_amount, unpack_token_amount, Close, Deposit,
    FranklinPriorityOp, FullExit, Transfer, Withdraw,
};
use crate::params::FR_ADDRESS_LEN;
use crate::primitives::{
    big_decimal_to_u128, bytes32_from_slice, bytes_slice_to_uint128, bytes_slice_to_uint32,
    u128_to_bigdecimal,
};
use bigdecimal::BigDecimal;
use ethabi::{decode, ParamType};
use std::convert::TryFrom;
use web3::types::{Address, U256};

pub const TX_TYPE_BYTES_LENGTH: usize = 1;
pub const ACCOUNT_ID_BYTES_LENGTH: usize = 3;
pub const TOKEN_BYTES_LENGTH: usize = 2;
pub const FULL_AMOUNT_BYTES_LENGTH: usize = 16;
pub const FEE_BYTES_LENGTH: usize = 2;
pub const ETH_ADDR_BYTES_LENGTH: usize = 20;
pub const PACKED_AMOUNT_BYTES_LENGTH: usize = 3;
pub const NONCE_BYTES_LENGTH: usize = 4;
pub const SIGNATURE_R_BYTES_LENGTH: usize = 32;
pub const SIGNATURE_S_BYTES_LENGTH: usize = 32;
pub const PUBKEY_PACKED_BYTES_LENGTH: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositOp {
    pub priority_op: Deposit,
    pub account_id: AccountId,
}

impl DepositOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x01;
    pub const OP_LENGTH: usize = 41;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        data.extend_from_slice(&self.priority_op.token.to_be_bytes());
        data.extend_from_slice(&big_decimal_to_u128(&self.priority_op.amount).to_be_bytes());
        data.extend_from_slice(&self.priority_op.account.data);
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::OP_LENGTH {
            return None;
        }
        let pre_length = 0;
        Some(Self {
            priority_op: Deposit::from_bytes(bytes)?,
            account_id: bytes_slice_to_uint32(
                &bytes[pre_length..pre_length + ACCOUNT_ID_BYTES_LENGTH],
            )?,
        })
        // let mut dec = decode(
        //     &[
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),  // account id
        //         ParamType::Uint(TOKEN_BYTES_LENGTH * 8),       // token
        //         ParamType::Uint(FULL_AMOUNT_BYTES_LENGTH * 8), // full amount
        //         ParamType::FixedBytes(FR_ADDRESS_LEN),         // new pubkey hash
        //     ],
        //     &bytes,
        // )
        // .ok()?;

        // let acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // let token = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u16::try_from(
        //         U256::as_u32(ui)
        //     ).ok()
        // )??;

        // let amount = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u128_to_bigdecimal(U256::as_u128(ui))
        // )?;

        // let address = AccountAddress::from_bytes(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )
        // .ok()?;

        // Some(Self {
        //     priority_op: Deposit {
        //         sender: Address::zero(), // In current circuit there is no sender in deposit pubdata
        //         token: token,
        //         amount: amount,
        //         account: address
        //     },
        //     account_id: acc_id,
        // })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoopOp {}

impl NoopOp {
    pub const CHUNKS: usize = 1;
    pub const OP_CODE: u8 = 0x00;
    pub const OP_LENGTH: usize = 0;

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes != [0, 0, 0, 0, 0, 0, 0, 0] {
            return None;
        }
        Some(Self {})
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferToNewOp {
    pub tx: Transfer,
    pub from: AccountId,
    pub to: AccountId,
}

impl TransferToNewOp {
    pub const CHUNKS: usize = 5;
    pub const OP_CODE: u8 = 0x02;
    pub const OP_LENGTH: usize = 33;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.from.to_be_bytes()[1..]);
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&pack_token_amount(&self.tx.amount));
        data.extend_from_slice(&self.tx.to.data);
        data.extend_from_slice(&self.to.to_be_bytes()[1..]);
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::OP_LENGTH {
            return None;
        }
        let from_pre_length = 0;
        let to_pre_length = ACCOUNT_ID_BYTES_LENGTH
            + TOKEN_BYTES_LENGTH
            + PACKED_AMOUNT_BYTES_LENGTH
            + FR_ADDRESS_LEN;
        Some(Self {
            tx: Transfer::from_transfer_to_new_bytes(bytes)?,
            from: bytes_slice_to_uint32(
                &bytes[from_pre_length..from_pre_length + ACCOUNT_ID_BYTES_LENGTH],
            )?,
            to: bytes_slice_to_uint32(
                &bytes[to_pre_length..to_pre_length + ACCOUNT_ID_BYTES_LENGTH],
            )?,
        })
        // let mut dec = decode(
        //     &[
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),       // from account id
        //         ParamType::Uint(TOKEN_BYTES_LENGTH * 8),            // token
        //         ParamType::Uint(FULL_AMOUNT_BYTES_LENGTH * 8),      // amount
        //         ParamType::FixedBytes(PACKED_AMOUNT_BYTES_LENGTH),  // new pubkey hash
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),       // to account id
        //         ParamType::FixedBytes(FEE_BYTES_LENGTH),            // fee
        //     ],
        //     &bytes,
        // )
        // .ok()?;

        // let from_acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // let token = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u16::try_from(
        //         U256::as_u32(ui)
        //     ).ok()
        // )??;

        // let amount = unpack_token_amount(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )?;

        // let to_address = AccountAddress::from_bytes(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )
        // .ok()?;

        // let to_acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // let fee = unpack_fee_amount(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )?;

        // Some(Self {
        //     tx: Transfer {
        //         from: AccountAddress::zero(),      // From pubdata its unknown
        //         to: to_address,
        //         token: token,
        //         amount: amount,
        //         fee: fee,
        //         nonce: 0,                          // From pubdata its unknown
        //         signature: TxSignature::default(), // From pubdata its unknown
        //     },
        //     from: from_acc_id,
        //     to: to_acc_id,
        // })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOp {
    pub tx: Transfer,
    pub from: AccountId,
    pub to: AccountId,
}

impl TransferOp {
    pub const CHUNKS: usize = 2;
    pub const OP_CODE: u8 = 0x05;
    pub const OP_LENGTH: usize = 13;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.from.to_be_bytes()[1..]);
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.to.to_be_bytes()[1..]);
        data.extend_from_slice(&pack_token_amount(&self.tx.amount));
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::OP_LENGTH {
            return None;
        }
        let from_pre_length = 0;
        let to_pre_length = ACCOUNT_ID_BYTES_LENGTH + TOKEN_BYTES_LENGTH;
        Some(Self {
            tx: Transfer::from_transfer_bytes(bytes)?,
            from: bytes_slice_to_uint32(
                &bytes[from_pre_length..from_pre_length + ACCOUNT_ID_BYTES_LENGTH],
            )?,
            to: bytes_slice_to_uint32(
                &bytes[to_pre_length..to_pre_length + ACCOUNT_ID_BYTES_LENGTH],
            )?,
        })
        // let mut dec = decode(
        //     &[
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),       // from account id
        //         ParamType::Uint(TOKEN_BYTES_LENGTH * 8),            // token
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),       // to account id
        //         ParamType::FixedBytes(PACKED_AMOUNT_BYTES_LENGTH),  // amount
        //         ParamType::FixedBytes(FEE_BYTES_LENGTH),            // fee
        //     ],
        //     &bytes,
        // )
        // .ok()?;

        // let from_acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // let token = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u16::try_from(
        //         U256::as_u32(ui)
        //     ).ok()
        // )??;

        // let to_acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // let amount = unpack_token_amount(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )?;

        // let fee = unpack_fee_amount(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )?;

        // Some(Self {
        //     tx: Transfer {
        //         from: AccountAddress::zero(),      // From pubdata its unknown
        //         to: AccountAddress::zero(),        // From pubdata its unknown
        //         token: token,
        //         amount: amount,
        //         fee: fee,
        //         nonce: 0,                          // From pubdata its unknown
        //         signature: TxSignature::default(), // From pubdata its unknown
        //     },
        //     from: from_acc_id,
        //     to: to_acc_id,
        // })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithdrawOp {
    pub tx: Withdraw,
    pub account_id: AccountId,
}

impl WithdrawOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x03;
    pub const OP_LENGTH: usize = 43;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&big_decimal_to_u128(&self.tx.amount).to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.extend_from_slice(self.tx.eth_address.as_bytes());
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::OP_LENGTH {
            return None;
        }
        let pre_length = 0;
        Some(Self {
            tx: Withdraw::from_bytes(bytes)?,
            account_id: bytes_slice_to_uint32(
                &bytes[pre_length..pre_length + ACCOUNT_ID_BYTES_LENGTH],
            )?,
        })
        // let mut dec = decode(
        //     &[
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),       // account id
        //         ParamType::Uint(TOKEN_BYTES_LENGTH * 8),            // token
        //         ParamType::Uint(FULL_AMOUNT_BYTES_LENGTH * 8),      // amount
        //         ParamType::FixedBytes(FEE_BYTES_LENGTH),            // fee
        //         ParamType::Address,                                 // eth address
        //     ],
        //     &bytes,
        // )
        // .ok()?;

        // let acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // let token = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u16::try_from(
        //         U256::as_u32(ui)
        //     ).ok()
        // )??;

        // let amount = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u128_to_bigdecimal(U256::as_u128(ui))
        // )?;

        // let fee = unpack_fee_amount(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )?;

        // let eth_address = dec
        // .remove(0)
        // .to_address()?;

        // Some(Self {
        //     tx: Withdraw {
        //         account: AccountAddress::zero(),   // From pubdata its unknown
        //         eth_address: eth_address,
        //         token: token,
        //         amount: amount,
        //         fee: fee,
        //         nonce: 0,                          // From pubdata its unknown
        //         signature: TxSignature::default(), // From pubdata its unknown
        //     },
        //     account_id: acc_id,
        // })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloseOp {
    pub tx: Close,
    pub account_id: AccountId,
}

impl CloseOp {
    pub const CHUNKS: usize = 1;
    pub const OP_CODE: u8 = 0x04;
    pub const OP_LENGTH: usize = 3;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::OP_LENGTH {
            return None;
        }
        let pre_length = 0;
        Some(Self {
            tx: Close::from_bytes(bytes)?,
            account_id: bytes_slice_to_uint32(
                &bytes[pre_length..pre_length + ACCOUNT_ID_BYTES_LENGTH],
            )?,
        })
        // let mut dec = decode(
        //     &[
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),       // account id
        //     ],
        //     &bytes,
        // )
        // .ok()?;

        // let acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // Some(Self {
        //     tx: Close{
        //         account: AccountAddress::zero(),   // From pubdata its unknown
        //         nonce: 0,                          // From pubdata its unknown
        //         signature: TxSignature::default(), // From pubdata its unknown
        //     },
        //     account_id: acc_id,
        // })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullExitOp {
    pub priority_op: FullExit,
    pub account_data: Option<(AccountId, BigDecimal)>,
}

impl FullExitOp {
    pub const CHUNKS: usize = 18;
    pub const OP_CODE: u8 = 0x06;
    pub const OP_LENGTH: usize = 141;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        let (account_id, amount) = self.account_data.clone().unwrap_or_default();
        data.extend_from_slice(&account_id.to_be_bytes()[1..]);
        data.extend_from_slice(&*self.priority_op.packed_pubkey);
        data.extend_from_slice(self.priority_op.eth_address.as_bytes());
        data.extend_from_slice(&self.priority_op.token.to_be_bytes());
        data.extend_from_slice(&self.priority_op.nonce.to_be_bytes());
        data.extend_from_slice(&*self.priority_op.signature_r);
        data.extend_from_slice(&*self.priority_op.signature_s);
        data.extend_from_slice(&big_decimal_to_u128(&amount).to_be_bytes());
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::OP_LENGTH {
            return None;
        }
        let acc_id_pre_length = 0;
        let to_pre_length = ACCOUNT_ID_BYTES_LENGTH
            + PUBKEY_PACKED_BYTES_LENGTH
            + ETH_ADDR_BYTES_LENGTH
            + TOKEN_BYTES_LENGTH
            + NONCE_BYTES_LENGTH
            + SIGNATURE_R_BYTES_LENGTH
            + SIGNATURE_S_BYTES_LENGTH;

        let acc_id = bytes_slice_to_uint32(
            &bytes[acc_id_pre_length..acc_id_pre_length + ACCOUNT_ID_BYTES_LENGTH],
        )?;
        let amount = u128_to_bigdecimal(bytes_slice_to_uint128(
            &bytes[to_pre_length..to_pre_length + FULL_AMOUNT_BYTES_LENGTH],
        )?);

        Some(Self {
            priority_op: FullExit::from_bytes(bytes)?,
            account_data: Some((acc_id, amount)),
        })
        // let mut dec = decode(
        //     &[
        //         ParamType::Uint(ACCOUNT_ID_BYTES_LENGTH * 8),      // account id
        //         ParamType::FixedBytes(PUBKEY_PACKED_BYTES_LENGTH), // pubkey
        //         ParamType::Address,                                // eth address
        //         ParamType::Uint(TOKEN_BYTES_LENGTH * 8),           // token
        //         ParamType::Uint(NONCE_BYTES_LENGTH * 8),           // nonce
        //         ParamType::FixedBytes(SIGNATURE_R_BYTES_LENGTH),   // sig r
        //         ParamType::FixedBytes(SIGNATURE_S_BYTES_LENGTH),   // sig s
        //         ParamType::Uint(FULL_AMOUNT_BYTES_LENGTH * 8),     // full amount
        //     ],
        //     &bytes,
        // )
        // .ok()?;

        // let acc_id = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(|ui| U256::as_u32(ui))?;

        // let pubkey = Box::from(bytes32_from_slice(
        //     dec
        //     .remove(0)
        //     .to_bytes()?
        //     .as_slice()
        // )?);

        // let eth_address = dec
        // .remove(0)
        // .to_address()?;

        // let token = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u16::try_from(
        //         U256::as_u32(ui)
        //     ).ok()
        // )??;

        // let nonce = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| U256::as_u32(ui)
        // )?;

        // let sig_r = Box::from(bytes32_from_slice(
        //     dec
        //         .remove(0)
        //         .to_bytes()?
        //         .as_slice()
        // )?);

        // let sig_s = Box::from(bytes32_from_slice(
        //     dec
        //         .remove(0)
        //         .to_bytes()?
        //         .as_slice()
        // )?);

        // let amount = dec
        // .remove(0)
        // .to_uint()
        // .as_ref()
        // .map(
        //     |ui| u128_to_bigdecimal(U256::as_u128(ui))
        // )?;

        // Some(Self {
        //     priority_op: FullExit {
        //         packed_pubkey: pubkey,
        //         eth_address: eth_address,
        //         token: token,
        //         nonce: nonce,
        //         signature_r: sig_r,
        //         signature_s: sig_s,
        //     },
        //     account_data: Some((acc_id, amount)),
        // })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinOp {
    Noop(NoopOp),
    Deposit(DepositOp),
    TransferToNew(TransferToNewOp),
    Withdraw(WithdrawOp),
    Close(CloseOp),
    Transfer(TransferOp),
    FullExit(FullExitOp),
}

impl FranklinOp {
    pub fn chunks(&self) -> usize {
        match self {
            FranklinOp::Noop(_) => NoopOp::CHUNKS,
            FranklinOp::Deposit(_) => DepositOp::CHUNKS,
            FranklinOp::TransferToNew(_) => TransferToNewOp::CHUNKS,
            FranklinOp::Withdraw(_) => WithdrawOp::CHUNKS,
            FranklinOp::Close(_) => CloseOp::CHUNKS,
            FranklinOp::Transfer(_) => TransferOp::CHUNKS,
            FranklinOp::FullExit(_) => FullExitOp::CHUNKS,
        }
    }

    pub fn public_data(&self) -> Vec<u8> {
        match self {
            FranklinOp::Noop(_) => vec![],
            FranklinOp::Deposit(op) => op.get_public_data(),
            FranklinOp::TransferToNew(op) => op.get_public_data(),
            FranklinOp::Withdraw(op) => op.get_public_data(),
            FranklinOp::Close(op) => op.get_public_data(),
            FranklinOp::Transfer(op) => op.get_public_data(),
            FranklinOp::FullExit(op) => op.get_public_data(),
        }
    }

    pub fn chunks_by_op_number(op_type: &u8) -> Option<usize> {
        match *op_type {
            NoopOp::OP_CODE => Some(NoopOp::CHUNKS),
            DepositOp::OP_CODE => Some(DepositOp::CHUNKS),
            TransferToNewOp::OP_CODE => Some(TransferToNewOp::CHUNKS),
            WithdrawOp::OP_CODE => Some(WithdrawOp::CHUNKS),
            CloseOp::OP_CODE => Some(CloseOp::CHUNKS),
            TransferOp::OP_CODE => Some(TransferOp::CHUNKS),
            FullExitOp::OP_CODE => Some(FullExitOp::CHUNKS),
            _ => None,
        }
    }

    pub fn from_bytes(op_type: &u8, bytes: &[u8]) -> Option<Self> {
        match *op_type {
            NoopOp::OP_CODE => Some(FranklinOp::Noop(NoopOp::from_bytes(&bytes)?)),
            DepositOp::OP_CODE => Some(FranklinOp::Deposit(DepositOp::from_bytes(&bytes)?)),
            TransferToNewOp::OP_CODE => Some(FranklinOp::TransferToNew(
                TransferToNewOp::from_bytes(&bytes)?,
            )),
            WithdrawOp::OP_CODE => Some(FranklinOp::Withdraw(WithdrawOp::from_bytes(&bytes)?)),
            CloseOp::OP_CODE => Some(FranklinOp::Close(CloseOp::from_bytes(&bytes)?)),
            TransferOp::OP_CODE => Some(FranklinOp::Transfer(TransferOp::from_bytes(&bytes)?)),
            FullExitOp::OP_CODE => Some(FranklinOp::FullExit(FullExitOp::from_bytes(&bytes)?)),
            _ => None,
        }
    }

    pub fn public_data_length(op_type: &u8) -> Option<usize> {
        match *op_type {
            NoopOp::OP_CODE => Some(NoopOp::OP_LENGTH),
            DepositOp::OP_CODE => Some(DepositOp::OP_LENGTH),
            TransferToNewOp::OP_CODE => Some(TransferToNewOp::OP_LENGTH),
            WithdrawOp::OP_CODE => Some(WithdrawOp::OP_LENGTH),
            CloseOp::OP_CODE => Some(CloseOp::OP_LENGTH),
            TransferOp::OP_CODE => Some(TransferOp::OP_LENGTH),
            FullExitOp::OP_CODE => Some(FullExitOp::OP_LENGTH),
            _ => None,
        }
    }

    pub fn try_get_tx(&self) -> Option<FranklinTx> {
        match self {
            FranklinOp::Transfer(op) => Some(FranklinTx::Transfer(op.tx.clone())),
            FranklinOp::TransferToNew(op) => Some(FranklinTx::Transfer(op.tx.clone())),
            FranklinOp::Withdraw(op) => Some(FranklinTx::Withdraw(op.tx.clone())),
            FranklinOp::Close(op) => Some(FranklinTx::Close(op.tx.clone())),
            _ => None,
        }
    }

    pub fn try_get_priority_op(&self) -> Option<FranklinPriorityOp> {
        match self {
            FranklinOp::Deposit(op) => Some(FranklinPriorityOp::Deposit(op.priority_op.clone())),
            FranklinOp::FullExit(op) => Some(FranklinPriorityOp::FullExit(op.priority_op.clone())),
            _ => None,
        }
    }
}
