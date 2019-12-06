use super::account::AccountAddress;
use super::tx::TxSignature;
use super::AccountId;
use super::FranklinTx;
use crate::node::{
    pack_fee_amount, pack_token_amount, unpack_fee_amount, unpack_token_amount, Close, Deposit,
    FranklinPriorityOp, FullExit, Transfer, Withdraw,
};
use crate::params::{
    FR_ADDRESS_LEN, ACCOUNT_ID_BIT_WIDTH,
    TOKEN_BIT_WIDTH, BALANCE_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH,
    FEE_EXPONENT_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH,
    NONCE_BIT_WIDTH, ETHEREUM_KEY_BIT_WIDTH, SIGNATURE_S_BIT_WIDTH_PADDED,
    SIGNATURE_R_BIT_WIDTH_PADDED, SUBTREE_HASH_WIDTH_PADDED
};
use crate::primitives::{
    big_decimal_to_u128, bytes32_from_slice, bytes_slice_to_uint128, bytes_slice_to_uint16,
    bytes_slice_to_uint32, u128_to_bigdecimal,
};
use bigdecimal::BigDecimal;
use web3::types::Address;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepositOp {
    pub priority_op: Deposit,
    pub account_id: AccountId,
}

impl DepositOp {
    pub const CHUNKS: usize = 6;
    pub const OP_CODE: u8 = 0x01;

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
        if bytes.len() != Self::CHUNKS * 8 {
            return None;
        }
        let account_id_pre_length = 1;
        let token_id_pre_length = account_id_pre_length + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_pre_length = token_id_pre_length + TOKEN_BIT_WIDTH / 8;
        let account_address_pre_length = amount_pre_length + BALANCE_BIT_WIDTH / 8;

        let account_id = bytes_slice_to_uint32(
            &bytes[account_id_pre_length..account_id_pre_length + ACCOUNT_ID_BIT_WIDTH / 8],
        )?;
        let token = bytes_slice_to_uint16(
            &bytes[token_id_pre_length..token_id_pre_length + TOKEN_BIT_WIDTH / 8],
        )?;
        let amount = u128_to_bigdecimal(bytes_slice_to_uint128(
            &bytes[amount_pre_length..amount_pre_length + BALANCE_BIT_WIDTH / 8],
        )?);
        let account = AccountAddress::from_bytes(
            &bytes[account_address_pre_length..account_address_pre_length + FR_ADDRESS_LEN],
        )
        .ok()?;
        let sender = Address::zero(); // In current circuit there is no sender in deposit pubdata

        Some(Self {
            priority_op: Deposit {
                sender,
                token,
                amount,
                account,
            },
            account_id,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoopOp {}

impl NoopOp {
    pub const CHUNKS: usize = 1;
    pub const OP_CODE: u8 = 0x00;

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes != [0, 0, 0, 0, 0, 0, 0, 0] {
            return None;
        }
        Some(Self {})
    }

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
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

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::CHUNKS * 8 {
            return None;
        }
        let from_pre_length = 1;
        let token_id_pre_length = from_pre_length + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_pre_length = token_id_pre_length + TOKEN_BIT_WIDTH / 8;
        let to_address_pre_length = amount_pre_length + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8;
        let to_id_pre_length = to_address_pre_length + FR_ADDRESS_LEN;
        let fee_pre_length = to_id_pre_length + ACCOUNT_ID_BIT_WIDTH / 8;

        let from_id = bytes_slice_to_uint32(
            &bytes[from_pre_length..from_pre_length + ACCOUNT_ID_BIT_WIDTH / 8],
        )?;
        let to_id = bytes_slice_to_uint32(
            &bytes[to_id_pre_length..to_id_pre_length + ACCOUNT_ID_BIT_WIDTH / 8],
        )?;
        let from_address = AccountAddress::zero(); // It is unknown from pubdata;
        let to_address = AccountAddress::from_bytes(
            &bytes[to_address_pre_length..to_address_pre_length + FR_ADDRESS_LEN],
        )
        .ok()?;
        let token = bytes_slice_to_uint16(
            &bytes[token_id_pre_length..token_id_pre_length + TOKEN_BIT_WIDTH / 8],
        )?;
        let amount = unpack_token_amount(
            &bytes[amount_pre_length..amount_pre_length + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8],
        )?;
        let fee = unpack_fee_amount(&bytes[fee_pre_length..fee_pre_length + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8])?;
        let nonce = 0; // It is unknown from pubdata
        let signature = TxSignature::default(); // It is unknown from pubdata

        Some(Self {
            tx: Transfer {
                from: from_address,
                to: to_address,
                token,
                amount,
                fee,
                nonce,
                signature,
            },
            from: from_id,
            to: to_id,
        })
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

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::CHUNKS * 8 {
            return None;
        }

        let from_pre_length = 1;
        let token_id_pre_length = from_pre_length + ACCOUNT_ID_BIT_WIDTH / 8;
        let to_pre_length = token_id_pre_length + TOKEN_BIT_WIDTH / 8;
        let amount_pre_length = to_pre_length + ACCOUNT_ID_BIT_WIDTH / 8;
        let fee_pre_length = amount_pre_length + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8;

        let from_address = AccountAddress::zero(); // From pubdata its unknown
        let to_address = AccountAddress::zero(); // From pubdata its unknown
        let token = bytes_slice_to_uint16(
            &bytes[token_id_pre_length..token_id_pre_length + TOKEN_BIT_WIDTH / 8],
        )?;
        let amount = unpack_token_amount(
            &bytes[amount_pre_length..amount_pre_length + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8],
        )?;
        let fee = unpack_fee_amount(&bytes[fee_pre_length..fee_pre_length + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8])?;
        let nonce = 0; // It is unknown from pubdata
        let signature = TxSignature::default(); // It is unknown from pubdata
        let from_id = bytes_slice_to_uint32(
            &bytes[from_pre_length..from_pre_length + ACCOUNT_ID_BIT_WIDTH / 8],
        )?;
        let to_id =
            bytes_slice_to_uint32(&bytes[to_pre_length..to_pre_length + ACCOUNT_ID_BIT_WIDTH / 8])?;

        Some(Self {
            tx: Transfer {
                from: from_address,
                to: to_address,
                token,
                amount,
                fee,
                nonce,
                signature,
            },
            from: from_id,
            to: to_id,
        })
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
        if bytes.len() != Self::CHUNKS * 8 {
            return None;
        }
        let account_pre_length = 1;
        let token_id_pre_length = account_pre_length + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_pre_length = token_id_pre_length + TOKEN_BIT_WIDTH / 8;
        let fee_pre_length = amount_pre_length + BALANCE_BIT_WIDTH / 8;
        let eth_address_pre_length = fee_pre_length + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8;

        let account_id = bytes_slice_to_uint32(
            &bytes[account_pre_length..account_pre_length + ACCOUNT_ID_BIT_WIDTH / 8],
        )?;
        let account_address = AccountAddress::zero(); // From pubdata it is unknown
        let token = bytes_slice_to_uint16(
            &bytes[token_id_pre_length..token_id_pre_length + TOKEN_BIT_WIDTH / 8],
        )?;
        let eth_address = Address::from_slice(
            &bytes[eth_address_pre_length..eth_address_pre_length + ETHEREUM_KEY_BIT_WIDTH / 8],
        );
        let amount = u128_to_bigdecimal(bytes_slice_to_uint128(
            &bytes[amount_pre_length..amount_pre_length + BALANCE_BIT_WIDTH / 8],
        )?);
        let fee = unpack_fee_amount(&bytes[fee_pre_length..fee_pre_length + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8])?;
        let nonce = 0; // From pubdata it is unknown
        let signature = TxSignature::default(); // From pubdata it is unknown

        Some(Self {
            tx: Withdraw {
                account: account_address,
                eth_address,
                token,
                amount,
                fee,
                nonce,
                signature,
            },
            account_id,
        })
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

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::CHUNKS * 8 {
            return None;
        }
        let account_id_pre_length = 1;
        let account_id = bytes_slice_to_uint32(
            &bytes[account_id_pre_length..account_id_pre_length + ACCOUNT_ID_BIT_WIDTH / 8],
        )?;
        let account_address = AccountAddress::zero(); // From pubdata it is unknown
        let nonce = 0; // From pubdata it is unknown
        let signature = TxSignature::default(); // From pubdata it is unknown
        Some(Self {
            tx: Close {
                account: account_address,
                nonce,
                signature,
            },
            account_id,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FullExitOp {
    pub priority_op: FullExit,
    /// None if withdraw was unsuccessful
    pub withdraw_amount: Option<BigDecimal>,
}

impl FullExitOp {
    pub const CHUNKS: usize = 18;
    pub const OP_CODE: u8 = 0x06;

    fn get_public_data(&self) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(Self::OP_CODE); // opcode
        data.extend_from_slice(&self.priority_op.account_id.to_be_bytes()[1..]);
        data.extend_from_slice(&*self.priority_op.packed_pubkey);
        data.extend_from_slice(self.priority_op.eth_address.as_bytes());
        data.extend_from_slice(&self.priority_op.token.to_be_bytes());
        data.extend_from_slice(&self.priority_op.nonce.to_be_bytes());
        data.extend_from_slice(&*self.priority_op.signature_r);
        data.extend_from_slice(&*self.priority_op.signature_s);
        data.extend_from_slice(
            &big_decimal_to_u128(&self.withdraw_amount.clone().unwrap_or_default()).to_be_bytes(),
        );
        data.resize(Self::CHUNKS * 8, 0x00);
        data
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::CHUNKS * 8 {
            return None;
        }

        let account_id_pre_length = 1;
        let packed_pubkey_pre_length = account_id_pre_length + ACCOUNT_ID_BIT_WIDTH / 8;
        let eth_address_pre_length = packed_pubkey_pre_length + SUBTREE_HASH_WIDTH_PADDED / 8;
        let token_pre_length = eth_address_pre_length + ETHEREUM_KEY_BIT_WIDTH / 8;
        let nonce_pre_length = token_pre_length + TOKEN_BIT_WIDTH / 8;
        let signature_r_pre_length = nonce_pre_length + NONCE_BIT_WIDTH / 8;
        let signature_s_pre_length = signature_r_pre_length + SIGNATURE_R_BIT_WIDTH_PADDED / 8;
        let amount_pre_length = signature_s_pre_length + SIGNATURE_S_BIT_WIDTH_PADDED / 8;

        let account_id = bytes_slice_to_uint32(
            &bytes[account_id_pre_length..account_id_pre_length + ACCOUNT_ID_BIT_WIDTH / 8],
        )?;
        let packed_pubkey = Box::from(bytes32_from_slice(
            &bytes[packed_pubkey_pre_length..packed_pubkey_pre_length + SUBTREE_HASH_WIDTH_PADDED / 8],
        )?);
        let eth_address = Address::from_slice(
            &bytes[eth_address_pre_length..eth_address_pre_length + ETHEREUM_KEY_BIT_WIDTH / 8],
        );
        let token =
            bytes_slice_to_uint16(&bytes[token_pre_length..token_pre_length + TOKEN_BIT_WIDTH / 8])?;
        let nonce =
            bytes_slice_to_uint32(&bytes[nonce_pre_length..nonce_pre_length + NONCE_BIT_WIDTH / 8])?;
        let signature_r = Box::from(bytes32_from_slice(
            &bytes[signature_r_pre_length..signature_r_pre_length + SIGNATURE_R_BIT_WIDTH_PADDED / 8],
        )?);
        let signature_s = Box::from(bytes32_from_slice(
            &bytes[signature_s_pre_length..signature_s_pre_length + SIGNATURE_S_BIT_WIDTH_PADDED / 8],
        )?);
        let amount = u128_to_bigdecimal(bytes_slice_to_uint128(
            &bytes[amount_pre_length..amount_pre_length + BALANCE_BIT_WIDTH / 8],
        )?);

        // If full exit amount is 0 - full exit is considered failed
        let withdraw_amount = if amount == BigDecimal::from(0) {
            None
        } else {
            Some(amount)
        };

        Some(Self {
            priority_op: FullExit {
                account_id,
                packed_pubkey,
                eth_address,
                token,
                nonce,
                signature_r,
                signature_s,
            },
            withdraw_amount,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinOp {
    Noop(NoopOp),
    Deposit(Box<DepositOp>),
    TransferToNew(Box<TransferToNewOp>),
    Withdraw(Box<WithdrawOp>),
    Close(Box<CloseOp>),
    Transfer(Box<TransferOp>),
    FullExit(Box<FullExitOp>),
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
            FranklinOp::Noop(op) => op.get_public_data(),
            FranklinOp::Deposit(op) => op.get_public_data(),
            FranklinOp::TransferToNew(op) => op.get_public_data(),
            FranklinOp::Withdraw(op) => op.get_public_data(),
            FranklinOp::Close(op) => op.get_public_data(),
            FranklinOp::Transfer(op) => op.get_public_data(),
            FranklinOp::FullExit(op) => op.get_public_data(),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let op_type: u8 = bytes[0];
        match op_type {
            NoopOp::OP_CODE => Some(FranklinOp::Noop(NoopOp::from_bytes(&bytes)?)),
            DepositOp::OP_CODE => Some(FranklinOp::Deposit(Box::new(DepositOp::from_bytes(
                &bytes,
            )?))),
            TransferToNewOp::OP_CODE => Some(FranklinOp::TransferToNew(Box::new(
                TransferToNewOp::from_bytes(&bytes)?,
            ))),
            WithdrawOp::OP_CODE => Some(FranklinOp::Withdraw(Box::new(WithdrawOp::from_bytes(
                &bytes,
            )?))),
            CloseOp::OP_CODE => Some(FranklinOp::Close(Box::new(CloseOp::from_bytes(&bytes)?))),
            TransferOp::OP_CODE => Some(FranklinOp::Transfer(Box::new(TransferOp::from_bytes(
                &bytes,
            )?))),
            FullExitOp::OP_CODE => Some(FranklinOp::FullExit(Box::new(FullExitOp::from_bytes(
                &bytes,
            )?))),
            _ => None,
        }
    }

    pub fn public_data_length(op_type: u8) -> Option<usize> {
        match op_type {
            NoopOp::OP_CODE => Some(NoopOp::CHUNKS * 8),
            DepositOp::OP_CODE => Some(DepositOp::CHUNKS * 8),
            TransferToNewOp::OP_CODE => Some(TransferToNewOp::CHUNKS * 8),
            WithdrawOp::OP_CODE => Some(WithdrawOp::CHUNKS * 8),
            CloseOp::OP_CODE => Some(CloseOp::CHUNKS * 8),
            TransferOp::OP_CODE => Some(TransferOp::CHUNKS * 8),
            FullExitOp::OP_CODE => Some(FullExitOp::CHUNKS * 8),
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
