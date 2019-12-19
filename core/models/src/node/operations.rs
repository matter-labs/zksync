use super::account::AccountAddress;
use super::tx::TxSignature;
use super::AccountId;
use super::FranklinTx;
use crate::node::{
    pack_fee_amount, pack_token_amount, unpack_fee_amount, unpack_token_amount, Close, Deposit,
    FranklinPriorityOp, FullExit, Transfer, Withdraw,
};
use crate::params::{
    ACCOUNT_ID_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, BALANCE_BIT_WIDTH,
    ETHEREUM_KEY_BIT_WIDTH, FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, FR_ADDRESS_LEN,
    NONCE_BIT_WIDTH, SIGNATURE_R_BIT_WIDTH_PADDED, SIGNATURE_S_BIT_WIDTH_PADDED,
    SUBTREE_HASH_WIDTH_PADDED, TOKEN_BIT_WIDTH,
};
use crate::primitives::{
    big_decimal_to_u128, bytes32_from_slice, bytes_slice_to_uint128, bytes_slice_to_uint16,
    bytes_slice_to_uint32, u128_to_bigdecimal,
};
use bigdecimal::BigDecimal;
use failure::{ensure, format_err};
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * 8,
            "Wrong bytes length for deposit pubdata"
        );

        let account_id_offset = 1;
        let token_id_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let account_address_offset = amount_offset + BALANCE_BIT_WIDTH / 8;

        let account_id = bytes_slice_to_uint32(
            &bytes[account_id_offset..account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or_else(|| format_err!("Cant get account id from deposit pubdata"))?;
        let token =
            bytes_slice_to_uint16(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get token id from deposit pubdata"))?;
        let amount = u128_to_bigdecimal(
            bytes_slice_to_uint128(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get amount from deposit pubdata"))?,
        );
        let account = AccountAddress::from_bytes(
            &bytes[account_address_offset..account_address_offset + FR_ADDRESS_LEN],
        )?;
        let sender = Address::zero(); // In current circuit there is no sender in deposit pubdata

        Ok(Self {
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes == [0, 0, 0, 0, 0, 0, 0, 0],
            "Wrong pubdata for noop operation"
        );
        Ok(Self {})
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * 8,
            "Wrong bytes length for transfer to new pubdata"
        );

        let from_offset = 1;
        let token_id_offset = from_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let to_address_offset =
            amount_offset + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8;
        let to_id_offset = to_address_offset + FR_ADDRESS_LEN;
        let fee_offset = to_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;

        let from_id =
            bytes_slice_to_uint32(&bytes[from_offset..from_offset + ACCOUNT_ID_BIT_WIDTH / 8])
                .ok_or_else(|| {
                    format_err!("Cant get from account id from transfer to new pubdata")
                })?;
        let to_id =
            bytes_slice_to_uint32(&bytes[to_id_offset..to_id_offset + ACCOUNT_ID_BIT_WIDTH / 8])
                .ok_or_else(|| {
                    format_err!("Cant get to account id from transfer to new pubdata")
                })?;
        let from_address = AccountAddress::zero(); // It is unknown from pubdata;
        let to_address = AccountAddress::from_bytes(
            &bytes[to_address_offset..to_address_offset + FR_ADDRESS_LEN],
        )?;
        let token =
            bytes_slice_to_uint16(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get token id from transfer to new pubdata"))?;
        let amount = unpack_token_amount(
            &bytes[amount_offset
                ..amount_offset + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get amount from transfer to new pubdata"))?;
        let fee = unpack_fee_amount(
            &bytes[fee_offset..fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get fee from transfer to new pubdata"))?;
        let nonce = 0; // It is unknown from pubdata
        let signature = TxSignature::default(); // It is unknown from pubdata

        Ok(Self {
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * 8,
            "Wrong bytes length for transfer pubdata"
        );

        let from_offset = 1;
        let token_id_offset = from_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let to_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let amount_offset = to_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let fee_offset =
            amount_offset + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8;

        let from_address = AccountAddress::zero(); // From pubdata its unknown
        let to_address = AccountAddress::zero(); // From pubdata its unknown
        let token =
            bytes_slice_to_uint16(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get token id from transfer pubdata"))?;
        let amount = unpack_token_amount(
            &bytes[amount_offset
                ..amount_offset + (AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get amount from transfer pubdata"))?;
        let fee = unpack_fee_amount(
            &bytes[fee_offset..fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get fee from transfer pubdata"))?;
        let nonce = 0; // It is unknown from pubdata
        let signature = TxSignature::default(); // It is unknown from pubdata
        let from_id =
            bytes_slice_to_uint32(&bytes[from_offset..from_offset + ACCOUNT_ID_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get from account id from transfer pubdata"))?;
        let to_id = bytes_slice_to_uint32(&bytes[to_offset..to_offset + ACCOUNT_ID_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get to account id from transfer pubdata"))?;

        Ok(Self {
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * 8,
            "Wrong bytes length for withdraw pubdata"
        );

        let account_offset = 1;
        let token_id_offset = account_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let amount_offset = token_id_offset + TOKEN_BIT_WIDTH / 8;
        let fee_offset = amount_offset + BALANCE_BIT_WIDTH / 8;
        let eth_address_offset = fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8;

        let account_id = bytes_slice_to_uint32(
            &bytes[account_offset..account_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or_else(|| format_err!("Cant get account id from withdraw pubdata"))?;
        let account_address = AccountAddress::zero(); // From pubdata it is unknown
        let token =
            bytes_slice_to_uint16(&bytes[token_id_offset..token_id_offset + TOKEN_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get token id from withdraw pubdata"))?;
        let eth_address = Address::from_slice(
            &bytes[eth_address_offset..eth_address_offset + ETHEREUM_KEY_BIT_WIDTH / 8],
        );
        let amount = u128_to_bigdecimal(
            bytes_slice_to_uint128(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get amount from withdraw pubdata"))?,
        );
        let fee = unpack_fee_amount(
            &bytes[fee_offset..fee_offset + (FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH) / 8],
        )
        .ok_or_else(|| format_err!("Cant get fee from withdraw pubdata"))?;
        let nonce = 0; // From pubdata it is unknown
        let signature = TxSignature::default(); // From pubdata it is unknown

        Ok(Self {
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * 8,
            "Wrong bytes length for close pubdata"
        );

        let account_id_offset = 1;
        let account_id = bytes_slice_to_uint32(
            &bytes[account_id_offset..account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or_else(|| format_err!("Cant get from account id from close pubdata"))?;
        let account_address = AccountAddress::zero(); // From pubdata it is unknown
        let nonce = 0; // From pubdata it is unknown
        let signature = TxSignature::default(); // From pubdata it is unknown
        Ok(Self {
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(
            bytes.len() == Self::CHUNKS * 8,
            "Wrong bytes length for full exit pubdata"
        );

        let account_id_offset = 1;
        let packed_pubkey_offset = account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8;
        let eth_address_offset = packed_pubkey_offset + SUBTREE_HASH_WIDTH_PADDED / 8;
        let token_offset = eth_address_offset + ETHEREUM_KEY_BIT_WIDTH / 8;
        let nonce_offset = token_offset + TOKEN_BIT_WIDTH / 8;
        let signature_r_offset = nonce_offset + NONCE_BIT_WIDTH / 8;
        let signature_s_offset = signature_r_offset + SIGNATURE_R_BIT_WIDTH_PADDED / 8;
        let amount_offset = signature_s_offset + SIGNATURE_S_BIT_WIDTH_PADDED / 8;

        let account_id = bytes_slice_to_uint32(
            &bytes[account_id_offset..account_id_offset + ACCOUNT_ID_BIT_WIDTH / 8],
        )
        .ok_or_else(|| format_err!("Cant get account id from full exit pubdata"))?;
        let packed_pubkey = Box::from(
            bytes32_from_slice(
                &bytes[packed_pubkey_offset..packed_pubkey_offset + SUBTREE_HASH_WIDTH_PADDED / 8],
            )
            .ok_or_else(|| format_err!("Cant get packed pubkey from full exit pubdata"))?,
        );
        let eth_address = Address::from_slice(
            &bytes[eth_address_offset..eth_address_offset + ETHEREUM_KEY_BIT_WIDTH / 8],
        );
        let token = bytes_slice_to_uint16(&bytes[token_offset..token_offset + TOKEN_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get token id from full exit pubdata"))?;
        let nonce = bytes_slice_to_uint32(&bytes[nonce_offset..nonce_offset + NONCE_BIT_WIDTH / 8])
            .ok_or_else(|| format_err!("Cant get nonce from full exit pubdata"))?;
        let signature_r = Box::from(
            bytes32_from_slice(
                &bytes[signature_r_offset..signature_r_offset + SIGNATURE_R_BIT_WIDTH_PADDED / 8],
            )
            .ok_or_else(|| format_err!("Cant get signature r from full exit pubdata"))?,
        );
        let signature_s = Box::from(
            bytes32_from_slice(
                &bytes[signature_s_offset..signature_s_offset + SIGNATURE_S_BIT_WIDTH_PADDED / 8],
            )
            .ok_or_else(|| format_err!("Cant get signature s from full exit pubdata"))?,
        );
        let amount = u128_to_bigdecimal(
            bytes_slice_to_uint128(&bytes[amount_offset..amount_offset + BALANCE_BIT_WIDTH / 8])
                .ok_or_else(|| format_err!("Cant get amount from full exit pubdata"))?,
        );

        // If full exit amount is 0 - full exit is considered failed
        let withdraw_amount = if amount == BigDecimal::from(0) {
            None
        } else {
            Some(amount)
        };

        Ok(Self {
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

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, failure::Error> {
        let op_type: u8 = *bytes.first().ok_or_else(|| format_err!("Empty pubdata"))?;
        match op_type {
            NoopOp::OP_CODE => Ok(FranklinOp::Noop(NoopOp::from_public_data(&bytes)?)),
            DepositOp::OP_CODE => Ok(FranklinOp::Deposit(Box::new(DepositOp::from_public_data(
                &bytes,
            )?))),
            TransferToNewOp::OP_CODE => Ok(FranklinOp::TransferToNew(Box::new(
                TransferToNewOp::from_public_data(&bytes)?,
            ))),
            WithdrawOp::OP_CODE => Ok(FranklinOp::Withdraw(Box::new(
                WithdrawOp::from_public_data(&bytes)?,
            ))),
            CloseOp::OP_CODE => Ok(FranklinOp::Close(Box::new(CloseOp::from_public_data(
                &bytes,
            )?))),
            TransferOp::OP_CODE => Ok(FranklinOp::Transfer(Box::new(
                TransferOp::from_public_data(&bytes)?,
            ))),
            FullExitOp::OP_CODE => Ok(FranklinOp::FullExit(Box::new(
                FullExitOp::from_public_data(&bytes)?,
            ))),
            _ => Err(format_err!("Wrong operation type: {}", &op_type)),
        }
    }

    pub fn public_data_length(op_type: u8) -> Result<usize, failure::Error> {
        match op_type {
            NoopOp::OP_CODE => Ok(NoopOp::CHUNKS * 8),
            DepositOp::OP_CODE => Ok(DepositOp::CHUNKS * 8),
            TransferToNewOp::OP_CODE => Ok(TransferToNewOp::CHUNKS * 8),
            WithdrawOp::OP_CODE => Ok(WithdrawOp::CHUNKS * 8),
            CloseOp::OP_CODE => Ok(CloseOp::CHUNKS * 8),
            TransferOp::OP_CODE => Ok(TransferOp::CHUNKS * 8),
            FullExitOp::OP_CODE => Ok(FullExitOp::CHUNKS * 8),
            _ => Err(format_err!("Wrong operation type: {}", &op_type)),
        }
    }

    pub fn try_get_tx(&self) -> Result<FranklinTx, failure::Error> {
        match self {
            FranklinOp::Transfer(op) => Ok(FranklinTx::Transfer(op.tx.clone())),
            FranklinOp::TransferToNew(op) => Ok(FranklinTx::Transfer(op.tx.clone())),
            FranklinOp::Withdraw(op) => Ok(FranklinTx::Withdraw(op.tx.clone())),
            FranklinOp::Close(op) => Ok(FranklinTx::Close(op.tx.clone())),
            _ => Err(format_err!("Wrong tx type")),
        }
    }

    pub fn try_get_priority_op(&self) -> Result<FranklinPriorityOp, failure::Error> {
        match self {
            FranklinOp::Deposit(op) => Ok(FranklinPriorityOp::Deposit(op.priority_op.clone())),
            FranklinOp::FullExit(op) => Ok(FranklinPriorityOp::FullExit(op.priority_op.clone())),
            _ => Err(format_err!("Wrong operation type")),
        }
    }
}
