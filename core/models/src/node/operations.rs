use super::AccountId;
use super::{pack_fee_amount, pack_token_amount, Deposit, FullExit};
use super::{Close, Transfer, Withdraw};
use crate::primitives::big_decimal_to_u128;
use bigdecimal::BigDecimal;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositOp {
    pub priority_op: Deposit,
    pub account_id: AccountId,
}

impl DepositOp {
    pub const CHUNKS: usize = 6;
    const OP_CODE: u8 = 0x01;

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
pub struct WithdrawOp {
    pub tx: Withdraw,
    pub account_id: AccountId,
}

impl WithdrawOp {
    pub const CHUNKS: usize = 6;
    const OP_CODE: u8 = 0x03;

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
pub struct FullExitOp {
    pub priority_op: FullExit,
    pub account_data: Option<(AccountId, BigDecimal)>,
}

impl FullExitOp {
    pub const CHUNKS: usize = 18;
    const OP_CODE: u8 = 0x06;

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinOp {
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
            FranklinOp::Deposit(op) => op.get_public_data(),
            FranklinOp::TransferToNew(op) => op.get_public_data(),
            FranklinOp::Withdraw(op) => op.get_public_data(),
            FranklinOp::Close(op) => op.get_public_data(),
            FranklinOp::Transfer(op) => op.get_public_data(),
            FranklinOp::FullExit(op) => op.get_public_data(),
        }
    }
}
