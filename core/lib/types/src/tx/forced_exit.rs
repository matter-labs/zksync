use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    AccountId, Nonce, TokenId,
};
use num::BigUint;

use crate::account::PubKeyHash;
use crate::Engine;
use anyhow::bail;
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::franklin_crypto::eddsa::PrivateKey;
use zksync_crypto::params::{max_account_id, max_token_id};
use zksync_utils::BigUintSerdeAsRadix10Str;

use super::{TxSignature, VerifiedSignatureCache};

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
        signature: Option<TxSignature>,
    ) -> Self {
        let mut tx = Self {
            initiator_account_id,
            target,
            token,
            fee,
            nonce,
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
    pub fn new_signed(
        initiator_account_id: AccountId,
        target: Address,
        token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, anyhow::Error> {
        let mut tx = Self::new(initiator_account_id, target, token, fee, nonce, None);
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if !tx.check_correctness() {
            bail!("Transfer is incorrect, check amounts");
        }
        Ok(tx)
    }

    /// Encodes the transaction data as the byte sequence according to the zkSync protocol.
    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.initiator_account_id.to_be_bytes());
        out.extend_from_slice(&self.target.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    /// Verifies the transaction correctness:
    ///
    /// - `initiator_account_id` field must be within supported range.
    /// - `token` field must be within supported range.
    /// - `fee` field must represent a packable value.
    /// - zkSync signature must correspond to the PubKeyHash of the account.
    pub fn check_correctness(&mut self) -> bool {
        let mut valid = is_fee_amount_packable(&self.fee)
            && self.initiator_account_id <= max_account_id()
            && self.token <= max_token_id();

        if valid {
            let signer = self.verify_signature();
            valid = valid && signer.is_some();
            self.cached_signer = VerifiedSignatureCache::Cached(signer);
        }
        valid
    }

    /// Restores the `PubKeyHash` from the transaction signature.
    pub fn verify_signature(&self) -> Option<PubKeyHash> {
        if let VerifiedSignatureCache::Cached(cached_signer) = &self.cached_signer {
            cached_signer.clone()
        } else if let Some(pub_key) = self.signature.verify_musig(&self.get_bytes()) {
            Some(PubKeyHash::from_pubkey(&pub_key))
        } else {
            None
        }
    }
}
