use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    AccountId, Nonce, TokenId,
};
use num::{BigUint, ToPrimitive};

use crate::account::PubKeyHash;
use crate::Engine;
use failure::bail;
use serde::{Deserialize, Serialize};
use zksync_basic_types::Address;
use zksync_crypto::franklin_crypto::eddsa::PrivateKey;
use zksync_crypto::params::{max_account_id, max_token_id};
use zksync_utils::format_units;
use zksync_utils::BigUintSerdeAsRadix10Str;

use super::{TxSignature, VerifiedSignatureCache};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdraw {
    pub account_id: AccountId,
    pub from: Address,
    pub to: Address,
    pub token: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub amount: BigUint,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    pub nonce: Nonce,
    pub signature: TxSignature,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
    /// Optional setting signalizing state keeper to speed up creation
    /// of the block with provided transaction.
    #[serde(default)]
    pub fast: bool,
}

impl Withdraw {
    pub const TX_TYPE: u8 = 3;

    #[allow(clippy::too_many_arguments)]
    /// Creates transaction from parts
    /// signature is optional, because sometimes we don't know it (i.e. data_restore)
    pub fn new(
        account_id: AccountId,
        from: Address,
        to: Address,
        token: TokenId,
        amount: BigUint,
        fee: BigUint,
        nonce: Nonce,
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
        };
        if signature.is_some() {
            tx.cached_signer = VerifiedSignatureCache::Cached(tx.verify_signature());
        }
        tx
    }

    #[allow(clippy::too_many_arguments)]
    /// Creates signed transaction using private key, checks for correcteness
    pub fn new_signed(
        account_id: AccountId,
        from: Address,
        to: Address,
        token: TokenId,
        amount: BigUint,
        fee: BigUint,
        nonce: Nonce,
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, failure::Error> {
        let mut tx = Self::new(account_id, from, to, token, amount, fee, nonce, None);
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if !tx.check_correctness() {
            bail!("Transfer is incorrect, check amounts");
        }
        Ok(tx)
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(&self.from.as_bytes());
        out.extend_from_slice(self.to.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&self.amount.to_u128().unwrap().to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    pub fn check_correctness(&mut self) -> bool {
        let mut valid = self.amount <= BigUint::from(u128::max_value())
            && is_fee_amount_packable(&self.fee)
            && self.account_id <= max_account_id()
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

    /// Get message that should be signed by Ethereum keys of the account for 2F authentication.
    pub fn get_ethereum_sign_message(&self, token_symbol: &str, decimals: u8) -> String {
        format!(
            "Withdraw {amount} {token}\n\
            To: {to:?}\n\
            Nonce: {nonce}\n\
            Fee: {fee} {token}\n\
            Account Id: {account_id}",
            amount = format_units(&self.amount, decimals),
            token = token_symbol,
            to = self.to,
            nonce = self.nonce,
            fee = format_units(&self.fee, decimals),
            account_id = self.account_id,
        )
    }
}
