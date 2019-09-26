use crate::params::FR_ADDRESS_LEN;
use super::AccountId;
use super::FranklinTx;
use super::{pack_fee_amount, pack_token_amount, Deposit, FullExit};
use super::{Close, Transfer, Withdraw};
use crate::node::FranklinPriorityOp;
use crate::primitives::big_decimal_to_u128;
use bigdecimal::BigDecimal;

pub const DEPOSIT_OP_LENGTH: usize = 43;
pub const TRANSFER_TO_NEW_OP_LENGTH: usize = 33;
pub const WITHDRAW_OP_LENGTH: usize = 43;
pub const CLOSE_OP_LENGTH: usize = 3;
pub const TRANSFER_OP_LENGTH: usize = 13;
pub const FULL_EXIT_OP_LENGTH: usize = 141;

pub const DEPOSIT_OP_CODE: usize = 1;
pub const TRANSFER_TO_NEW_OP_CODE: usize = 2;
pub const WITHDRAW_OP_CODE: usize = 3;
pub const CLOSE_OP_CODE: usize = 4;
pub const TRANSFER_OP_CODE: usize = 5;
pub const FULL_EXIT_OP_CODE: usize = 6;

pub const TX_TYPE_BYTES_LEGTH: usize = 1;
pub const ACCOUNT_ID_BYTES_LEGTH: usize = 3;
pub const TOKEN_BYTES_LENGTH: usize = 2;
pub const FULL_AMOUNT_BYTES_LEGTH: usize = 16;
pub const FEE_BYTES_LEGTH: usize = 2;
pub const ETH_ADDR_BYTES_LEGTH: usize = 20;
pub const PACKED_AMOUNT_BYTES_LEGTH: usize = 3;
pub const NONCE_BYTES_LEGTH: usize = 4;
pub const SIGNATURE_R_BYTES_LEGTH: usize = 32;
pub const SIGNATURE_S_BYTES_LEGTH: usize = 32;
pub const PUBKEY_PACKED_BYTES_LEGTH: usize = 32;


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

    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let pre_length = 0;
        Self {
            priority_op: Deposit::from_bytes(bytes),
            account_id: AccountId::from_be_bytes(bytes[pre_length .. pre_length + ACCOUNT_ID_BYTES_LEGTH])
        }
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

    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let from_pre_length = 0;
        let to_pre_length = ACCOUNT_ID_BYTES_LEGTH +
            TOKEN_BYTES_LENGTH +
            PACKED_AMOUNT_BYTES_LEGTH +
            FR_ADDRESS_LEN;
        Self {
            tx: Transfer::from_transfer_to_new_bytes(bytes),
            from: AccountId::from_be_bytes(bytes[from_pre_length .. from_pre_length + ACCOUNT_ID_BYTES_LEGTH]),
            to: AccountId::from_be_bytes(bytes[to_pre_length .. to_pre_length + ACCOUNT_ID_BYTES_LEGTH])
        }
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

    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let from_pre_length = 0;
        let to_pre_length = ACCOUNT_ID_BYTES_LEGTH +
            TOKEN_BYTES_LENGTH;
        Self {
            tx: Transfer::from_transfer_bytes(bytes),
            from: AccountId::from_be_bytes(bytes[from_pre_length .. from_pre_length + ACCOUNT_ID_BYTES_LEGTH]),
            to: AccountId::from_be_bytes(bytes[to_pre_length .. to_pre_length + ACCOUNT_ID_BYTES_LEGTH])
        }
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

    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let pre_length = 0;
        Self {
            tx: Withdraw::from_bytes(bytes),
            account_id: AccountId::from_be_bytes(bytes[pre_length..pre_length + ACCOUNT_ID_BYTES_LEGTH])
        }
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

    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let pre_length = 0;
        Self {
            tx: Close::from_bytes(bytes),
            account_id: AccountId::from_be_bytes(bytes[pre_length .. pre_length + ACCOUNT_ID_BYTES_LEGTH])
        }
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

    pub fn from_bytes(bytes: &Vec<u8>) -> Self {
        let acc_id_pre_length = 0;
        let to_pre_length = ACCOUNT_ID_BYTES_LEGTH +
            PUBKEY_PACKED_BYTES_LEGTH +
            ETH_ADDR_BYTES_LEGTH +
            TOKEN_BYTES_LENGTH +
            NONCE_BYTES_LEGTH +
            SIGNATURE_R_BYTES_LEGTH +
            SIGNATURE_S_BYTES_LEGTH;

        let acc_id = AccountId::from_be_bytes(bytes[acc_id_pre_length..acc_id_pre_length + ACCOUNT_ID_BYTES_LEGTH]);
        let amount = BigDecimal::parse_bytes(bytes[to_pre_length .. to_pre_length + FULL_AMOUNT_BYTES_LEGTH].to_vec(), 18);
        
        Self {
            priority_op: FullExit::from_bytes(bytes),
            account_data: Some((acc_id, amount))
        }
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

    pub fn chunks_by_op_number(op_type: &u8) -> Option<usize> {
        match *op_type {
            DEPOSIT_OP_CODE => Some(DepositOp::CHUNKS),
            TRANSFER_TO_NEW_OP_CODE => Some(TransferToNewOp::CHUNKS),
            WITHDRAW_OP_CODE => Some(WithdrawOp::CHUNKS),
            CLOSE_OP_CODE => Some(CloseOp::CHUNKS),
            TRANSFER_OP_CODE => Some(TransferOp::CHUNKS),
            FULL_EXIT_OP_CODE => Some(FullExitOp::CHUNKS),
            _ => None
        }
    }

    pub fn from_bytes(bytes: &Vec<u8>) -> Option<Self> {
        let op_type: &u8 = bytes[0];
        match *op_type {
            DEPOSIT_OP_CODE => Some(FranklinOp::Deposit(DepositOp::from_bytes(&bytes))),
            TRANSFER_TO_NEW_OP_CODE => Some(FranklinOp::TransferToNew(TransferToNewOp::from_bytes(&bytes))),
            WITHDRAW_OP_CODE => Some(FranklinOp::Withdraw(WithdrawOp::from_bytes(&bytes))),
            CLOSE_OP_CODE => Some(FranklinOp::Close(CloseOp::from_bytes(&bytes))),
            TRANSFER_OP_CODE => Some(FranklinOp::Transfer(TransferOp::from_bytes(&bytes))),
            FULL_EXIT_OP_CODE => Some(FranklinOp::FullExit(FullExitOp::from_bytes(&bytes))),
            _ => None
        }
    }

    pub fn public_data_length(bytes: &Vec<u8>) -> Option<u8> {
        let op_type: &u8 = bytes[0];
        match *op_type {
            DEPOSIT_OP_CODE => Some(DEPOSIT_OP_LENGTH),
            TRANSFER_TO_NEW_OP_CODE => Some(TRANSFER_TO_NEW_OP_LENGTH),
            WITHDRAW_OP_CODE => Some(WITHDRAW_OP_LENGTH),
            CLOSE_OP_CODE => Some(CLOSE_OP_LENGTH),
            TRANSFER_OP_CODE => Some(TRANSFER_OP_LENGTH),
            FULL_EXIT_OP_CODE => Some(FULL_EXIT_OP_LENGTH),
            _ => None
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
