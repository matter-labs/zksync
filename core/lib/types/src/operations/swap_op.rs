use num::Zero;
use serde::{Deserialize, Serialize};
use zksync_crypto::params::{
    ACCOUNT_ID_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH, AMOUNT_MANTISSA_BIT_WIDTH, CHUNK_BYTES,
    FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, TOKEN_BIT_WIDTH,
};
use zksync_crypto::primitives::FromBytes;

use crate::{
    helpers::{pack_fee_amount, pack_token_amount, unpack_fee_amount, unpack_token_amount},
    operations::error::SwapOpError,
    tx::Order,
    AccountId, Address, Nonce, Swap, TokenId,
};

/// Swap operation. For details, see the documentation of [`ZkSyncOp`](./operations/enum.ZkSyncOp.html).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapOp {
    pub tx: Swap,
    pub submitter: AccountId,
    pub accounts: (AccountId, AccountId),
    pub recipients: (AccountId, AccountId),
}

impl SwapOp {
    pub const CHUNKS: usize = 5;
    pub const OP_CODE: u8 = 0x0b;

    pub(crate) fn get_public_data(&self) -> Vec<u8> {
        let mut data = vec![Self::OP_CODE]; // opcode
        data.extend_from_slice(&self.accounts.0.to_be_bytes());
        data.extend_from_slice(&self.recipients.0.to_be_bytes());
        data.extend_from_slice(&self.accounts.1.to_be_bytes());
        data.extend_from_slice(&self.recipients.1.to_be_bytes());
        data.extend_from_slice(&self.submitter.to_be_bytes());
        data.extend_from_slice(&self.tx.orders.0.token_sell.to_be_bytes());
        data.extend_from_slice(&self.tx.orders.1.token_sell.to_be_bytes());
        data.extend_from_slice(&self.tx.fee_token.to_be_bytes());
        data.extend_from_slice(&pack_token_amount(&self.tx.amounts.0));
        data.extend_from_slice(&pack_token_amount(&self.tx.amounts.1));
        data.extend_from_slice(&pack_fee_amount(&self.tx.fee));
        let nonce_mask = (!self.tx.orders.0.amount.is_zero() as u8)
            + (!self.tx.orders.1.amount.is_zero() as u8) * 2;
        data.push(nonce_mask);
        data.resize(Self::CHUNKS * CHUNK_BYTES, 0x00);
        data
    }

    pub fn from_public_data(bytes: &[u8]) -> Result<Self, SwapOpError> {
        if bytes.len() != Self::CHUNKS * CHUNK_BYTES {
            return Err(SwapOpError::PubdataSizeMismatch);
        }

        const AMOUNT_BIT_WIDTH: usize = AMOUNT_EXPONENT_BIT_WIDTH + AMOUNT_MANTISSA_BIT_WIDTH;
        const FEE_BIT_WIDTH: usize = FEE_EXPONENT_BIT_WIDTH + FEE_MANTISSA_BIT_WIDTH;

        let accounts_offset = 1;
        let tokens_offset = accounts_offset + ACCOUNT_ID_BIT_WIDTH * 5 / 8;
        let amounts_offset = tokens_offset + ACCOUNT_ID_BIT_WIDTH * 3 / 8;
        let fee_offset = amounts_offset + AMOUNT_BIT_WIDTH * 2 / 8;

        let read_token = |offset| {
            u32::from_bytes(&bytes[offset..offset + TOKEN_BIT_WIDTH / 8])
                .ok_or(SwapOpError::CannotGetTokenId)
        };

        let read_account = |offset| {
            u32::from_bytes(&bytes[offset..offset + ACCOUNT_ID_BIT_WIDTH / 8])
                .ok_or(SwapOpError::CannotGetAccountId)
        };

        let read_amount = |offset| {
            unpack_token_amount(&bytes[offset..offset + AMOUNT_BIT_WIDTH / 8])
                .ok_or(SwapOpError::CannotGetAmount)
        };

        let fee = unpack_fee_amount(&bytes[fee_offset..fee_offset + FEE_BIT_WIDTH / 8])
            .ok_or(SwapOpError::CannotGetFee)?;
        let account_id_0 = AccountId(read_account(accounts_offset)?);
        let recipient_id_0 = AccountId(read_account(accounts_offset + ACCOUNT_ID_BIT_WIDTH / 8)?);
        let account_id_1 = AccountId(read_account(
            accounts_offset + ACCOUNT_ID_BIT_WIDTH * 2 / 8,
        )?);
        let recipient_id_1 = AccountId(read_account(
            accounts_offset + ACCOUNT_ID_BIT_WIDTH * 3 / 8,
        )?);
        let submitter_id = AccountId(read_account(
            accounts_offset + ACCOUNT_ID_BIT_WIDTH * 4 / 8,
        )?);
        let token_0 = TokenId(read_token(tokens_offset)?);
        let token_1 = TokenId(read_token(tokens_offset + TOKEN_BIT_WIDTH / 8)?);
        let fee_token = TokenId(read_token(tokens_offset + TOKEN_BIT_WIDTH * 2 / 8)?);
        let amount_0 = read_amount(amounts_offset)?;
        let amount_1 = read_amount(amounts_offset + AMOUNT_BIT_WIDTH / 8)?;
        let nonce = Nonce(0); // It is unknown from pubdata
        let nonce_mask = bytes[fee_offset + FEE_BIT_WIDTH / 8];

        let order_a = Order {
            account_id: account_id_0,
            nonce,
            recipient_address: Address::zero(), // unknown from pubdata
            // First bit indicates whether this amount is 0 or not.
            amount: amount_0.clone() * (nonce_mask & 1),
            token_buy: token_1,
            token_sell: token_0,
            time_range: Default::default(),
            signature: Default::default(),
            price: (amount_0.clone(), amount_1.clone()),
        };

        let order_b = Order {
            account_id: account_id_1,
            nonce,
            recipient_address: Address::zero(), // unknown from pubdata
            // Second bit indicates whether this amount is 0 or not,
            // there're only 2 bits in total.
            amount: amount_1.clone() * (nonce_mask >> 1),
            token_buy: token_0,
            token_sell: token_1,
            time_range: Default::default(),
            signature: Default::default(),
            price: (amount_1.clone(), amount_0.clone()),
        };

        Ok(Self {
            tx: Swap::new(
                submitter_id,
                Default::default(), // Address is unknown from pubdata
                nonce,
                (order_a, order_b),
                (amount_0, amount_1),
                fee,
                fee_token,
                None,
            ),
            submitter: submitter_id,
            accounts: (account_id_0, account_id_1),
            recipients: (recipient_id_0, recipient_id_1),
        })
    }

    pub fn get_updated_account_ids(&self) -> Vec<AccountId> {
        vec![
            self.submitter,
            self.accounts.0,
            self.accounts.1,
            self.recipients.0,
            self.recipients.1,
        ]
    }
}
