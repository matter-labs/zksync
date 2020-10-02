use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    AccountId, Nonce, TokenId,
};
use num::BigUint;

use crate::account::PubKeyHash;
use crate::Engine;
use failure::bail;
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::franklin_crypto::eddsa::PrivateKey;
use zksync_crypto::params::{max_account_id, max_token_id};
use zksync_utils::BigUintSerdeAsRadix10Str;

use super::{TxSignature, VerifiedSignatureCache};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForcedExit {
    /// Account ID of the transaction initiator.
    pub initiator_account_id: AccountId,
    /// Address of the account to withdraw funds from.
    pub target: Address,
    pub token: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
    pub signature: TxSignature,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
}

impl ForcedExit {
    const TX_TYPE: u8 = 8;

    /// Creates transaction from parts
    /// signature is optional, because sometimes we don't know it (i.e. data_restore)
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

    /// Creates signed transaction using private key, checks for correcteness
    pub fn new_signed(
        initiator_account_id: AccountId,
        target: Address,
        token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, failure::Error> {
        let mut tx = Self::new(initiator_account_id, target, token, fee, nonce, None);
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if !tx.check_correctness() {
            bail!("Transfer is incorrect, check amounts");
        }
        Ok(tx)
    }

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
