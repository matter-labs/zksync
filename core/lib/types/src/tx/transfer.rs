use crate::{
    helpers::{
        is_fee_amount_packable, is_token_amount_packable, pack_fee_amount, pack_token_amount,
    },
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
use zksync_utils::format_units;
use zksync_utils::BigUintSerdeAsRadix10Str;

use super::{TxSignature, VerifiedSignatureCache};

/// `Transfer` transaction performs a move of funds from one zkSync account to another.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Transfer {
    /// zkSync network account ID of the transaction initiator.
    pub account_id: AccountId,
    /// Address of account to transfer funds from.
    pub from: Address,
    /// Address of account to transfer funds to.
    pub to: Address,
    /// Type of token for transfer. Also represents the token in which fee will be paid.
    pub token: TokenId,
    /// Amount of funds to transfer.
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
}

impl Transfer {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 5;

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
        private_key: &PrivateKey<Engine>,
    ) -> Result<Self, anyhow::Error> {
        let mut tx = Self::new(account_id, from, to, token, amount, fee, nonce, None);
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
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(&self.from.as_bytes());
        out.extend_from_slice(&self.to.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_token_amount(&self.amount));
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    /// Verifies the transaction correctness:
    ///
    /// - `account_id` field must be within supported range.
    /// - `token` field must be within supported range.
    /// - `amount` field must represent a packable value.
    /// - `fee` field must represent a packable value.
    /// - transfer recipient must not be `Adddress::zero()`.
    /// - zkSync signature must correspond to the PubKeyHash of the account.
    pub fn check_correctness(&mut self) -> bool {
        let mut valid = self.amount <= BigUint::from(u128::max_value())
            && self.fee <= BigUint::from(u128::max_value())
            && is_token_amount_packable(&self.amount)
            && is_fee_amount_packable(&self.fee)
            && self.account_id <= max_account_id()
            && self.token <= max_token_id()
            && self.to != Address::zero();
        if valid {
            let signer = self.verify_signature();
            valid = valid && signer.is_some();
            self.cached_signer = VerifiedSignatureCache::Cached(signer);
        };
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

    /// Gets message that should be signed by Ethereum keys of the account for 2-Factor authentication.
    pub fn get_ethereum_sign_message(&self, token_symbol: &str, decimals: u8) -> String {
        format!(
            "Transfer {amount} {token}\n\
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
