use num::{BigUint, ToPrimitive, Zero};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use zksync_basic_types::Address;
use zksync_crypto::{
    franklin_crypto::eddsa::PrivateKey,
    params::{
        max_account_id, max_fungible_token_id, max_processable_token, CURRENT_TX_VERSION,
        MIN_NFT_TOKEN_ID,
    },
};
use zksync_utils::{format_units, BigUintSerdeAsRadix10Str};

use crate::{account::PubKeyHash, utils::ethereum_sign_message_part, Engine};
use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    tx::error::TransactionSignatureError,
    AccountId, Nonce, TokenId,
};

use super::{TimeRange, TxSignature, VerifiedSignatureCache};
use crate::tx::version::TxVersion;

/// `Withdraw` transaction performs a withdrawal of funds from zkSync account to L1 account.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdraw {
    /// zkSync network account ID of the transaction initiator.
    pub account_id: AccountId,
    /// Address of L2 account to withdraw funds from.
    pub from: Address,
    /// Address of L1 account to withdraw funds to.
    pub to: Address,
    /// Type of token for withdrawal. Also represents the token in which fee will be paid.
    pub token: TokenId,
    /// Amount of funds to withdraw.
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
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
    /// This fields must be Option<...> because of backward compatibility with first version of ZkSync
    #[serde(flatten)]
    pub time_range: Option<TimeRange>,
}

impl Withdraw {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 3;

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
        amount: BigUint,
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
            amount,
            fee,
            nonce,
            signature: signature.clone().unwrap_or_default(),
            cached_signer: VerifiedSignatureCache::NotCached,
            fast: false,
            time_range: Some(time_range),
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
        account_id: AccountId,
        from: Address,
        to: Address,
        token: TokenId,
        amount: BigUint,
        fee: BigUint,
        nonce: Nonce,
        time_range: TimeRange,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, TransactionSignatureError> {
        let mut tx = Self::new(
            account_id, from, to, token, amount, fee, nonce, time_range, None,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if let Err(err) = tx.check_correctness() {
            println!("Error {:?}", err);
            return Err(TransactionSignatureError);
        }
        Ok(tx)
    }

    pub fn is_backwards_compatible(&self) -> bool {
        self.token.0 < MIN_NFT_TOKEN_ID
    }

    /// Encodes the transaction data as the byte sequence according to the old zkSync protocol with 2 bytes token.
    pub fn get_old_bytes(&self) -> Vec<u8> {
        if !self.is_backwards_compatible() {
            return vec![];
        }
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(self.from.as_bytes());
        out.extend_from_slice(self.to.as_bytes());
        out.extend_from_slice(&(self.token.0 as u16).to_be_bytes());
        out.extend_from_slice(&self.amount.to_u128().unwrap().to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        if let Some(time_range) = &self.time_range {
            out.extend_from_slice(&time_range.as_be_bytes());
        }
        out
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
        out.extend_from_slice(&self.amount.to_u128().unwrap().to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        if let Some(time_range) = &self.time_range {
            out.extend_from_slice(&time_range.as_be_bytes());
        }
        out
    }

    /// Restores the `PubKeyHash` from the transaction signature.
    pub fn verify_signature(&self) -> Option<(PubKeyHash, TxVersion)> {
        if let VerifiedSignatureCache::Cached(cached_signer) = &self.cached_signer {
            *cached_signer
        } else {
            if self.token.0 < MIN_NFT_TOKEN_ID {
                if let Some(res) = self
                    .signature
                    .verify_musig(&self.get_old_bytes())
                    .map(|pub_key| PubKeyHash::from_pubkey(&pub_key))
                {
                    return Some((res, TxVersion::Legacy));
                }
            }
            self.signature
                .verify_musig(&self.get_bytes())
                .map(|pub_key| (PubKeyHash::from_pubkey(&pub_key), TxVersion::V1))
        }
    }

    /// Get the first part of the message we expect to be signed by Ethereum account key.
    /// The only difference is the missing `nonce` since it's added at the end of the transactions
    /// batch message.
    pub fn get_ethereum_sign_message_part(&self, token_symbol: &str, decimals: u8) -> String {
        ethereum_sign_message_part(
            "Withdraw",
            token_symbol,
            decimals,
            &self.amount,
            &self.fee,
            &self.to,
        )
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

    /// Returns an old-format message that should be signed by Ethereum account key.
    /// Needed for backwards compatibility.
    pub fn get_old_ethereum_sign_message(&self, token_symbol: &str, decimals: u8) -> String {
        format!(
            "Withdraw {amount} {token}\n\
            To: {to:?}\n\
            Nonce: {nonce}\n\
            Fee: {fee} {token}\n\
            Account Id: {account_id}",
            amount = format_units(&self.amount, decimals),
            token = token_symbol,
            to = self.to,
            nonce = *self.nonce,
            fee = format_units(&self.fee, decimals),
            account_id = *self.account_id,
        )
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
    ///
    /// Note that we don't need to check whether token amount is packable, because pubdata for this operation
    /// contains unpacked value only.
    pub fn check_correctness(&mut self) -> Result<(), TransactionError> {
        if self.amount > BigUint::from(u128::MAX) {
            return Err(TransactionError::WrongAmount);
        }
        if self.fee > BigUint::from(u128::MAX) {
            return Err(TransactionError::WrongFee);
        }
        if !is_fee_amount_packable(&self.fee) {
            return Err(TransactionError::FeeNotPackable);
        }
        if self.account_id > max_account_id() {
            return Err(TransactionError::WrongAccountId);
        }

        if self.token > max_fungible_token_id() {
            return Err(TransactionError::WrongToken);
        }
        if !self
            .time_range
            .map(|r| r.check_correctness())
            .unwrap_or(true)
        {
            return Err(TransactionError::WrongTimeRange);
        }

        // Fee can only be paid in processable tokens
        if self.fee != BigUint::zero() && self.token > max_processable_token() {
            return Err(TransactionError::WrongTokenForPayingFee);
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
    #[error("Wrong amount")]
    WrongAmount,
    #[error("Amount is not packable")]
    AmountNotPackable,
    #[error("Wrong fee")]
    WrongFee,
    #[error("Fee is not packable")]
    FeeNotPackable,
    #[error("Wrong account id")]
    WrongAccountId,
    #[error("Wrong token")]
    WrongToken,
    #[error("Wrong time range")]
    WrongTimeRange,
    #[error("Wrong signature")]
    WrongSignature,
    #[error("Wrong token for paying fees")]
    WrongTokenForPayingFee,
}
