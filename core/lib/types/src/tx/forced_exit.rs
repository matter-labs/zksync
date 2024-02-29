use num::{BigUint, Zero};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use thiserror::Error;

use zksync_basic_types::Address;
use zksync_crypto::{
    franklin_crypto::eddsa::PrivateKey,
    params::{max_account_id, max_fungible_token_id, max_processable_token, CURRENT_TX_VERSION},
};
use zksync_utils::{format_units, BigUintSerdeAsRadix10Str};

use super::{TxSignature, VerifiedSignatureCache};
use crate::tx::error::{
    FEE_AMOUNT_IS_NOT_PACKABLE, WRONG_ACCOUNT_ID, WRONG_FEE_ERROR, WRONG_SIGNATURE,
    WRONG_TIME_RANGE, WRONG_TOKEN, WRONG_TOKEN_FOR_PAYING_FEE,
};
use crate::tx::version::TxVersion;
use crate::tx::TimeRange;
use crate::{account::PubKeyHash, Engine};
use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    AccountId, Nonce, TokenId,
};

/// `ForcedExit` transaction is used to withdraw funds from an unowned
/// account to its corresponding L1 address.
///
/// Caller of this function will pay fee for the operation, and has no
/// control over the address on which funds will be withdrawn. Account
/// to which `ForcedExit` is applied must have no public key hash set.
///
/// This operation is expected to be used in cases when account in L1
/// cannot prove its identity in L2 (e.g. it's an existing smart contract),
/// so the funds won't get "locked" in L2.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExit {
    /// zkSync network account ID of the transaction initiator.
    pub initiator_account_id: AccountId,
    /// Address of the account to withdraw funds from.
    /// Also this field represents the address in L1 to which funds will be withdrawn.
    pub target: Address,
    /// Type of token for withdrawal. Also represents the token in which fee will be paid.
    pub token: TokenId,
    /// Fee for the transaction.
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    /// Current initiator account nonce.
    pub nonce: Nonce,
    /// Transaction zkSync signature.
    pub signature: TxSignature,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
    /// Time range when the transaction is valid
    /// This fields must be Option<...> because of backward compatibility with first version of ZkSync
    #[serde(flatten)]
    pub time_range: Option<TimeRange>,
}

impl ForcedExit {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 8;

    /// Creates transaction from all the required fields.
    ///
    /// While `signature` field is mandatory for new transactions, it may be `None`
    /// in some cases (e.g. when restoring the network state from the L1 contract data).
    pub fn new(
        initiator_account_id: AccountId,
        target: Address,
        token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        time_range: TimeRange,
        signature: Option<TxSignature>,
    ) -> Self {
        let mut tx = Self {
            initiator_account_id,
            target,
            token,
            fee,
            nonce,
            time_range: Some(time_range),
            signature: signature.clone().unwrap_or_default(),
            cached_signer: VerifiedSignatureCache::NotCached,
        };
        if signature.is_some() {
            tx.cached_signer = VerifiedSignatureCache::Cached(tx.verify_signature());
        }
        tx
    }

    /// Creates a signed transaction using private key and
    /// checks for the transaction correctness.
    pub fn new_signed(
        initiator_account_id: AccountId,
        target: Address,
        token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        time_range: TimeRange,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, TransactionError> {
        let mut tx = Self::new(
            initiator_account_id,
            target,
            token,
            fee,
            nonce,
            time_range,
            None,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        tx.check_correctness()?;
        Ok(tx)
    }

    /// Encodes the transaction data as the byte sequence according to the zkSync protocol.
    pub fn get_old_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.initiator_account_id.to_be_bytes());
        out.extend_from_slice(self.target.as_bytes());
        out.extend_from_slice(&(self.token.0 as u16).to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.time_range.unwrap_or_default().as_be_bytes());
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
        out.extend_from_slice(&self.initiator_account_id.to_be_bytes());
        out.extend_from_slice(self.target.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out.extend_from_slice(&self.time_range.unwrap_or_default().as_be_bytes());
        out
    }

    /// Restores the `PubKeyHash` from the transaction signature.
    pub fn verify_signature(&self) -> Option<(PubKeyHash, TxVersion)> {
        if let VerifiedSignatureCache::Cached(cached_signer) = &self.cached_signer {
            *cached_signer
        } else {
            if let Some(res) = self
                .signature
                .verify_musig(&self.get_old_bytes())
                .map(|pub_key| PubKeyHash::from_pubkey(&pub_key))
            {
                return Some((res, TxVersion::Legacy));
            }
            self.signature
                .verify_musig(&self.get_bytes())
                .map(|pub_key| (PubKeyHash::from_pubkey(&pub_key), TxVersion::V1))
        }
    }

    /// Get the first part of the message we expect to be signed by Ethereum account key.
    /// The only difference is the missing `nonce` since it's added at the end of the transactions
    /// batch message. The format is:
    ///
    /// ForcedExit {token} to: {target}
    /// [Fee: {fee} {token}]
    ///
    /// Note that the second line is optional.
    pub fn get_ethereum_sign_message_part(&self, token_symbol: &str, decimals: u8) -> String {
        let mut message = format!(
            "ForcedExit {token} to: {to:?}",
            token = token_symbol,
            to = self.target
        );
        if !self.fee.is_zero() {
            message.push_str(
                format!(
                    "\nFee: {fee} {token}",
                    fee = format_units(&self.fee, decimals),
                    token = token_symbol,
                )
                .as_str(),
            );
        }
        message
    }

    /// Gets message that should be signed by Ethereum keys of the account for 2-Factor authentication.
    pub fn get_ethereum_sign_message(&self, token_symbol: &str, decimals: u8) -> String {
        let mut message = self.get_ethereum_sign_message_part(token_symbol, decimals);
        message.push_str(format!("\nNonce: {}", self.nonce).as_str());
        message
    }

    /// Helper method to remove cache and test transaction behavior without the signature cache.
    #[doc(hidden)]
    pub fn wipe_signer_cache(&mut self) {
        self.cached_signer = VerifiedSignatureCache::NotCached;
    }

    /// Verifies the transaction correctness:
    ///
    /// - `initiator_account_id` field must be within supported range.
    /// - `token` field must be within supported range.
    /// - `fee` field must represent a packable value.
    /// - zkSync signature must correspond to the PubKeyHash of the account.
    pub fn check_correctness(&mut self) -> Result<(), TransactionError> {
        if self.fee > BigUint::from(u128::MAX) {
            return Err(TransactionError::WrongFee);
        }
        if !is_fee_amount_packable(&self.fee) {
            return Err(TransactionError::FeeNotPackable);
        }
        if self.initiator_account_id > max_account_id() {
            return Err(TransactionError::WrongInitiatorAccountId);
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
    WrongFee,
    FeeNotPackable,
    WrongInitiatorAccountId,
    WrongToken,
    WrongTimeRange,
    WrongSignature,
    WrongTokenForPayingFee,
}

impl Display for TransactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error = match self {
            TransactionError::WrongFee => WRONG_FEE_ERROR,
            TransactionError::FeeNotPackable => FEE_AMOUNT_IS_NOT_PACKABLE,
            TransactionError::WrongToken => WRONG_TOKEN,
            TransactionError::WrongTimeRange => WRONG_TIME_RANGE,
            TransactionError::WrongSignature => WRONG_SIGNATURE,
            TransactionError::WrongTokenForPayingFee => WRONG_TOKEN_FOR_PAYING_FEE,
            TransactionError::WrongInitiatorAccountId => WRONG_ACCOUNT_ID,
        };
        write!(f, "{}", error)
    }
}
