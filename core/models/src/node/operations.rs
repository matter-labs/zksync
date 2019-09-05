use super::tx::{Close, Deposit, Transfer, Withdraw};
use super::AccountId;
use crate::node::{pack_fee_amount, pack_token_amount};
use bigdecimal::ToPrimitive;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositOp {
    pub tx: Deposit,
    pub account_id: AccountId,
}

impl DepositOp {
    pub const CHUNKS: usize = 6;
    const OP_CODE: u8 = 0x01;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.tx.amount.to_u128().unwrap().to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.extend_from_slice(&self.tx.to.data);
        data.resize(Self::CHUNKS * 8, 0x00);
        data
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialExitOp {
    pub tx: Withdraw,
    pub account_id: AccountId,
}

impl PartialExitOp {
    pub const CHUNKS: usize = 6;
    const OP_CODE: u8 = 0x03;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        data.extend_from_slice(&self.tx.token.to_be_bytes());
        data.extend_from_slice(&self.tx.amount.to_u128().unwrap().to_be_bytes());
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        data.extend_from_slice(&self.tx.eth_address);
        data.resize(Self::CHUNKS * 8, 0x00);
        data
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

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.account_id.to_be_bytes()[1..]);
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinOp {
    Deposit(DepositOp),
    TransferToNew(TransferToNewOp),
    PartialExit(PartialExitOp),
    Close(CloseOp),
    Transfer(TransferOp),
}

impl FranklinOp {
    pub fn chunks(&self) -> usize {
        match self {
            FranklinOp::Deposit(_) => DepositOp::CHUNKS,
            FranklinOp::TransferToNew(_) => TransferToNewOp::CHUNKS,
            FranklinOp::PartialExit(_) => PartialExitOp::CHUNKS,
            FranklinOp::Close(_) => CloseOp::CHUNKS,
            FranklinOp::Transfer(_) => TransferOp::CHUNKS,
        }
    }

    pub fn public_data(&self) -> Vec<u8> {
        match self {
            FranklinOp::Deposit(op) => op.get_public_data(),
            FranklinOp::TransferToNew(op) => op.get_public_data(),
            FranklinOp::PartialExit(op) => op.get_public_data(),
            FranklinOp::Close(op) => op.get_public_data(),
            FranklinOp::Transfer(op) => op.get_public_data(),
        }
    }
}
