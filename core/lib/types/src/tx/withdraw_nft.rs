use num::{BigUint, Zero};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use thiserror::Error;

use zksync_crypto::{
    franklin_crypto::eddsa::PrivateKey,
    params::{
        max_account_id, max_processable_token, max_token_id, CURRENT_TX_VERSION, MIN_NFT_TOKEN_ID,
    },
};

use zksync_utils::{format_units, BigUintSerdeAsRadix10Str};

use super::{TimeRange, TxSignature, VerifiedSignatureCache};
use crate::tx::error::{
    FEE_AMOUNT_IS_NOT_PACKABLE, WRONG_ACCOUNT_ID, WRONG_FEE_ERROR, WRONG_SIGNATURE,
    WRONG_TIME_RANGE, WRONG_TOKEN, WRONG_TOKEN_FOR_PAYING_FEE,
};
use crate::tx::version::TxVersion;
use crate::{
    account::PubKeyHash,
    helpers::{is_fee_amount_packable, pack_fee_amount},
    AccountId, Address, Engine, Nonce, TokenId,
};

/// `Withdraw` transaction performs a withdrawal of funds from zkSync account to L1 account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WithdrawNFT {
    /// zkSync network account ID of the transaction initiator.
    pub account_id: AccountId,
    /// Address of L2 account to withdraw funds from.
    pub from: Address,
    /// Address of L1 account to withdraw funds to.
    pub to: Address,
    /// Type of token for withdrawal.
    pub token: TokenId,
    /// Token Fee for the transaction.
    pub fee_token: TokenId,
    /// Fee for the transaction.
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    /// Current account nonce.
    pub nonce: Nonce,
    /// Transaction zkSync signature.
    pub signature: TxSignature,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
    /// Optional setting signalizing state keeper to speed up creation
    /// of the block with provided transaction.
    /// This field is only set by the server. Transaction with this field set manually will be
    /// rejected.
    #[serde(default)]
    pub fast: bool,
    /// Time range when the transaction is valid
    #[serde(flatten)]
    pub time_range: TimeRange,
}

impl WithdrawNFT {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 10;

    /// Creates transaction from all the required fields.
    ///
    /// While `signature` field is mandatory for new transactions, it may be `None`
    /// in some cases (e.g. when restoring the network state from the L1 contract data).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account_id: AccountId,
        from: Address,
        to: Address,
        token: TokenId,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        time_range: TimeRange,
        signature: Option<TxSignature>,
    ) -> Self {
        let mut tx = Self {
            account_id,
            from,
            to,
            token,
            fee_token,
            fee,
            nonce,
            signature: signature.clone().unwrap_or_default(),
            cached_signer: VerifiedSignatureCache::NotCached,
            fast: false,
            time_range,
        };
        if signature.is_some() {
            tx.cached_signer = VerifiedSignatureCache::Cached(tx.verify_signature());
        }
        tx
    }

    /// Creates a signed transaction using private key and
    /// checks for the transaction correctness.
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        account_id: AccountId,
        from: Address,
        to: Address,
        token: TokenId,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        time_range: TimeRange,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, TransactionError> {
        let mut tx = Self::new(
            account_id, from, to, token, fee_token, fee, nonce, time_range, None,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        tx.check_correctness()?;
        Ok(tx)
    }

    /// Encodes the transaction data as the byte sequence according to the zkSync protocol.
    pub fn get_bytes(&self) -> Vec<u8> {
        self.get_bytes_with_version(CURRENT_TX_VERSION)
    }

    pub fn get_bytes_with_version(&self, version: u8) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[255u8 - Self::TX_TYPE]);
        out.extend_from_slice(&[version]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(self.from.as_bytes());
        out.extend_from_slice(self.to.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&self.fee_token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.time_range.as_be_bytes());
        out
    }

    /// Restores the `PubKeyHash` from the transaction signature.
    pub fn verify_signature(&self) -> Option<(PubKeyHash, TxVersion)> {
        if let VerifiedSignatureCache::Cached(cached_signer) = &self.cached_signer {
            *cached_signer
        } else {
            self.signature
                .verify_musig(&self.get_bytes())
                .map(|pub_key| (PubKeyHash::from_pubkey(&pub_key), TxVersion::V1))
        }
    }

    /// Get the first part of the message we expect to be signed by Ethereum account key.
    /// The only difference is the missing `nonce` since it's added at the end of the transactions
    /// batch message.
    pub fn get_ethereum_sign_message_part(&self, fee_token_symbol: &str, decimals: u8) -> String {
        let mut message = format!(
            "WithdrawNFT {token} to: {to:?}",
            token = self.token.0,
            to = self.to
        );
        if !self.fee.is_zero() {
            if !message.is_empty() {
                message.push('\n');
            }
            message.push_str(
                format!(
                    "Fee: {fee} {token}",
                    fee = format_units(self.fee.clone(), decimals),
                    token = fee_token_symbol
                )
                .as_str(),
            );
        }
        message
    }

    /// Get message that should be signed by Ethereum keys of the account for 2-Factor authentication.
    pub fn get_ethereum_sign_message(&self, token_symbol: &str, decimals: u8) -> String {
        let mut message = self.get_ethereum_sign_message_part(token_symbol, decimals);
        if !message.is_empty() {
            message.push('\n');
        }
        message.push_str(format!("Nonce: {}", self.nonce).as_str());
        message
    }

    /// Helper method to remove cache and test transaction behavior without the signature cache.
    #[doc(hidden)]
    pub fn wipe_signer_cache(&mut self) {
        self.cached_signer = VerifiedSignatureCache::NotCached;
    }

    /// Verifies the transaction correctness:
    ///
    /// - `account_id` field must be within supported range.
    /// - `token` field must be within supported range.
    /// - `fee` field must represent a packable value.
    /// - zkSync signature must correspond to the PubKeyHash of the account.
    pub fn check_correctness(&mut self) -> Result<(), TransactionError> {
        if self.fee > BigUint::from(u128::MAX) {
            return Err(TransactionError::WrongFee);
        }

        if self.token > max_token_id() && self.token < TokenId(MIN_NFT_TOKEN_ID) {
            return Err(TransactionError::WrongToken);
        }
        if !is_fee_amount_packable(&self.fee) {
            return Err(TransactionError::FeeNotPackable);
        }
        if self.account_id > max_account_id() {
            return Err(TransactionError::WrongAccountId);
        }

        if !self.time_range.check_correctness() {
            return Err(TransactionError::WrongTimeRange);
        }

        // Fee can only be paid in processable tokens
        if self.fee_token > max_processable_token() {
            return Err(TransactionError::WrongFeeToken);
        }

        let signer = self.verify_signature();
        self.cached_signer = VerifiedSignatureCache::Cached(signer);
        if signer.is_none() {
            return Err(TransactionError::WrongSignature);
        }
        Ok(())
    }
}

#[derive(Error, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TransactionError {
    WrongFee,
    FeeNotPackable,
    WrongAccountId,
    WrongToken,
    WrongTimeRange,
    WrongSignature,
    WrongFeeToken,
}

impl Display for TransactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error = match self {
            TransactionError::WrongFee => WRONG_FEE_ERROR,
            TransactionError::FeeNotPackable => FEE_AMOUNT_IS_NOT_PACKABLE,
            TransactionError::WrongAccountId => WRONG_ACCOUNT_ID,
            TransactionError::WrongToken => WRONG_TOKEN,
            TransactionError::WrongTimeRange => WRONG_TIME_RANGE,
            TransactionError::WrongSignature => WRONG_SIGNATURE,
            TransactionError::WrongFeeToken => WRONG_TOKEN_FOR_PAYING_FEE,
        };
        write!(f, "{}", error)
    }
}
