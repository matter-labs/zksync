use crate::account::PubKeyHash;
use crate::Engine;
use crate::{
    helpers::{
        is_fee_amount_packable, is_token_amount_packable, pack_fee_amount, pack_token_amount,
    },
    tx::TimeRange,
    AccountId, Nonce, TokenId,
};
use num::{BigUint, Zero};
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::{
    franklin_crypto::eddsa::PrivateKey,
    params::{
        max_account_id, max_processable_token, max_token_id, CURRENT_TX_VERSION, PRICE_BIT_WIDTH,
    },
    primitives::rescue_hash_orders,
};
use zksync_utils::{format_units, BigUintPairSerdeAsRadix10Str, BigUintSerdeAsRadix10Str};

use super::{TxSignature, VerifiedSignatureCache};
use crate::tx::error::TransactionSignatureError;
use crate::tx::version::TxVersion;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    pub account_id: AccountId,
    #[serde(rename = "recipient")]
    pub recipient_address: Address,
    pub nonce: Nonce,
    pub token_buy: TokenId,
    pub token_sell: TokenId,
    #[serde(rename = "ratio")]
    #[serde(with = "BigUintPairSerdeAsRadix10Str")]
    pub price: (BigUint, BigUint),
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    #[serde(flatten)]
    pub time_range: TimeRange,
    pub signature: TxSignature,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Swap {
    pub submitter_id: AccountId,
    pub submitter_address: Address,
    pub nonce: Nonce,
    pub orders: (Order, Order),
    #[serde(with = "BigUintPairSerdeAsRadix10Str")]
    pub amounts: (BigUint, BigUint),
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub fee_token: TokenId,
    pub signature: TxSignature,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
}

impl Order {
    /// Unique identifier of the signed message, similar to TX_TYPE
    pub const MSG_TYPE: u8 = b'o'; // 'o' for "order"

    /// Encodes the transaction data as the byte sequence according to the zkSync protocol.
    pub fn get_bytes(&self) -> Vec<u8> {
        self.get_bytes_with_version(CURRENT_TX_VERSION)
    }

    pub fn get_bytes_with_version(&self, version: u8) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::MSG_TYPE]);
        out.extend_from_slice(&[version]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(&self.recipient_address.as_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.token_sell.to_be_bytes());
        out.extend_from_slice(&self.token_buy.to_be_bytes());
        out.extend_from_slice(&pad_front(&self.price.0.to_bytes_be(), PRICE_BIT_WIDTH / 8));
        out.extend_from_slice(&pad_front(&self.price.1.to_bytes_be(), PRICE_BIT_WIDTH / 8));
        out.extend_from_slice(&pack_token_amount(&self.amount));
        out.extend_from_slice(&self.time_range.to_be_bytes());
        out
    }

    pub fn verify_signature(&self) -> Option<PubKeyHash> {
        self.signature
            .verify_musig(&self.get_bytes())
            .map(|pub_key| PubKeyHash::from_pubkey(&pub_key))
    }

    pub fn check_correctness(&self) -> bool {
        self.price.0.bits() as usize <= PRICE_BIT_WIDTH
            && self.price.1.bits() as usize <= PRICE_BIT_WIDTH
            && self.account_id <= max_account_id()
            && self.recipient_address != Address::zero()
            && self.token_buy <= max_token_id()
            && self.token_sell <= max_token_id()
            && self.time_range.check_correctness()
    }

    pub fn get_ethereum_sign_message(
        &self,
        token_sell: &str,
        token_buy: &str,
        decimals: u8,
    ) -> String {
        let mut message = if self.amount.is_zero() {
            format!("Limit order for {} -> {}\n", token_sell, token_buy)
        } else {
            format!(
                "Order for {} {} -> {}\n",
                format_units(&self.amount, decimals),
                token_sell,
                token_buy
            )
        };
        message += format!(
            "Ratio: {sell}:{buy}\n\
            Address: {recipient:?}\n\
            Nonce: {nonce}",
            sell = self.price.0.to_string(),
            buy = self.price.1.to_string(),
            recipient = self.recipient_address,
            nonce = self.nonce
        )
        .as_str();
        message
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        account_id: AccountId,
        recipient_address: Address,
        nonce: Nonce,
        token_sell: TokenId,
        token_buy: TokenId,
        price: (BigUint, BigUint),
        amount: BigUint,
        time_range: TimeRange,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, TransactionSignatureError> {
        let mut tx = Self {
            account_id,
            recipient_address,
            nonce,
            token_buy,
            token_sell,
            price,
            amount,
            time_range,
            signature: Default::default(),
        };
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if !tx.check_correctness() {
            return Err(TransactionSignatureError);
        }
        Ok(tx)
    }
}

impl Swap {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 11;

    /// Creates transaction from all the required fields.
    ///
    /// While `signature` field is mandatory for new transactions, it may be `None`
    /// in some cases (e.g. when restoring the network state from the L1 contract data).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        submitter_id: AccountId,
        submitter_address: Address,
        nonce: Nonce,
        orders: (Order, Order),
        amounts: (BigUint, BigUint),
        fee: BigUint,
        fee_token: TokenId,
        signature: Option<TxSignature>,
    ) -> Self {
        let mut tx = Self {
            submitter_id,
            submitter_address,
            nonce,
            orders,
            amounts,
            fee,
            fee_token,
            signature: signature.clone().unwrap_or_default(),
            cached_signer: VerifiedSignatureCache::NotCached,
        };
        if signature.is_some() {
            tx.cached_signer = VerifiedSignatureCache::Cached(tx.verify_signature());
        }
        tx
    }

    /// Creates a signed transaction using private key and
    /// checks for the transaction correcteness.
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        submitter_id: AccountId,
        submitter_address: Address,
        nonce: Nonce,
        orders: (Order, Order),
        amounts: (BigUint, BigUint),
        fee: BigUint,
        fee_token: TokenId,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, TransactionSignatureError> {
        let mut tx = Self::new(
            submitter_id,
            submitter_address,
            nonce,
            orders,
            amounts,
            fee,
            fee_token,
            None,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_sign_bytes());
        if !tx.check_correctness() {
            return Err(TransactionSignatureError);
        }
        Ok(tx)
    }

    /// Encodes the transaction data as the byte sequence according to the zkSync protocol.
    pub fn get_bytes(&self) -> Vec<u8> {
        let mut first_order_bytes = self.orders.0.get_bytes();
        let mut second_order_bytes = self.orders.1.get_bytes();
        let order_byte_size = first_order_bytes.len();

        let mut orders_bytes = Vec::with_capacity(order_byte_size * 2);
        orders_bytes.append(&mut first_order_bytes);
        orders_bytes.append(&mut second_order_bytes);

        self.get_swap_bytes(&orders_bytes)
    }

    /// Constructs the byte sequence to be signed for swap.
    /// It differs from `get_bytes`, because there we include all the data, including orders data,
    /// and here we represent orders by their hashes. This is required due to limited message size
    /// for which signatures can be verified in circuit.
    pub fn get_sign_bytes(&self) -> Vec<u8> {
        let mut first_order_bytes = self.orders.0.get_bytes();
        let mut second_order_bytes = self.orders.1.get_bytes();
        let order_byte_size = first_order_bytes.len();

        let mut orders_bytes = Vec::with_capacity(order_byte_size * 2);
        orders_bytes.append(&mut first_order_bytes);
        orders_bytes.append(&mut second_order_bytes);

        let orders_hash = rescue_hash_orders(&orders_bytes);
        self.get_swap_bytes(&orders_hash)
    }

    /// Encodes transaction data, using provided encoded data for orders.
    /// This function does not care how orders are encoded: is it data or hash.
    fn get_swap_bytes(&self, order_bytes: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[255u8 - Self::TX_TYPE]);
        out.extend_from_slice(&[CURRENT_TX_VERSION]);
        out.extend_from_slice(&self.submitter_id.to_be_bytes());
        out.extend_from_slice(&self.submitter_address.as_bytes());
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(order_bytes);
        out.extend_from_slice(&self.fee_token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&pack_token_amount(&self.amounts.0));
        out.extend_from_slice(&pack_token_amount(&self.amounts.1));
        out
    }

    fn check_amounts(&self) -> bool {
        self.amounts.0 <= BigUint::from(u128::max_value())
            && self.amounts.1 <= BigUint::from(u128::max_value())
            && is_token_amount_packable(&self.amounts.0)
            && is_token_amount_packable(&self.amounts.1)
            && is_fee_amount_packable(&self.fee)
    }

    pub fn valid_from(&self) -> u64 {
        std::cmp::max(
            self.orders.0.time_range.valid_from,
            self.orders.1.time_range.valid_from,
        )
    }

    pub fn valid_until(&self) -> u64 {
        std::cmp::min(
            self.orders.0.time_range.valid_until,
            self.orders.1.time_range.valid_until,
        )
    }

    pub fn time_range(&self) -> TimeRange {
        TimeRange::new(self.valid_from(), self.valid_until())
    }

    /// Verifies the transaction correctness:
    pub fn check_correctness(&mut self) -> bool {
        let mut valid = self.check_amounts()
            && self.submitter_id <= max_account_id()
            && self.fee_token <= max_processable_token()
            && self.orders.0.check_correctness()
            && self.orders.1.check_correctness()
            && self.time_range().check_correctness();
        if valid {
            let signer = self.verify_signature();
            valid = valid && signer.is_some();
            self.cached_signer = VerifiedSignatureCache::Cached(signer);
        };
        valid
    }

    /// Restores the `PubKeyHash` from the transaction signature.
    pub fn verify_signature(&self) -> Option<(PubKeyHash, TxVersion)> {
        if let VerifiedSignatureCache::Cached(cached_signer) = &self.cached_signer {
            *cached_signer
        } else {
            self.signature
                .verify_musig(&self.get_sign_bytes())
                .map(|pub_key| (PubKeyHash::from_pubkey(&pub_key), TxVersion::V1))
        }
    }

    /// Get the first part of the message we expect to be signed by Ethereum account key.
    /// The only difference is the missing `nonce` since it's added at the end of the transactions
    /// batch message.
    pub fn get_ethereum_sign_message_part(&self, token_symbol: &str, decimals: u8) -> String {
        if !self.fee.is_zero() {
            format!(
                "Swap fee: {fee} {token}",
                fee = format_units(&self.fee, decimals),
                token = token_symbol
            )
        } else {
            String::new()
        }
    }

    /// Gets message that should be signed by Ethereum keys of the account for 2-Factor authentication.
    pub fn get_ethereum_sign_message(&self, token_symbol: &str, decimals: u8) -> String {
        let mut message = self.get_ethereum_sign_message_part(token_symbol, decimals);
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(format!("Nonce: {}", self.nonce).as_str());
        message
    }
}

fn pad_front(bytes: &[u8], size: usize) -> Vec<u8> {
    assert!(size >= bytes.len());
    let mut result = vec![0u8; size];
    result[size - bytes.len()..].copy_from_slice(bytes);
    result
}
