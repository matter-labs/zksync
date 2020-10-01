use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    AccountId, Nonce,
};

use crate::account::PubKeyHash;
use failure::ensure;
use num::BigUint;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{Address, TokenId};
use zksync_crypto::{
    params::{max_account_id, max_token_id},
    PrivateKey,
};
use zksync_utils::BigUintSerdeAsRadix10Str;

use super::{PackedEthSignature, TxSignature, VerifiedSignatureCache};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKey {
    pub account_id: AccountId,
    pub account: Address,
    pub new_pk_hash: PubKeyHash,
    #[serde(default)]
    pub fee_token: TokenId,
    #[serde(with = "BigUintSerdeAsRadix10Str", default)]
    pub fee: BigUint,
    pub nonce: Nonce,
    pub signature: TxSignature,
    pub eth_signature: Option<PackedEthSignature>,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
}

impl ChangePubKey {
    const TX_TYPE: u8 = 7;

    /// Creates transaction from parts
    /// signature is optional, because sometimes we don't know it (i.e. data_restore)
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account_id: AccountId,
        account: Address,
        new_pk_hash: PubKeyHash,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        signature: Option<TxSignature>,
        eth_signature: Option<PackedEthSignature>,
    ) -> Self {
        let mut tx = Self {
            account_id,
            account,
            new_pk_hash,
            fee_token,
            fee,
            nonce,
            signature: signature.clone().unwrap_or_default(),
            eth_signature,
            cached_signer: VerifiedSignatureCache::NotCached,
        };
        if signature.is_some() {
            tx.cached_signer = VerifiedSignatureCache::Cached(tx.verify_signature());
        }
        tx
    }

    /// Creates signed transaction using private key, checks for correcteness
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        account_id: AccountId,
        account: Address,
        new_pk_hash: PubKeyHash,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        eth_signature: Option<PackedEthSignature>,
        private_key: &PrivateKey,
    ) -> Result<Self, failure::Error> {
        let mut tx = Self::new(
            account_id,
            account,
            new_pk_hash,
            fee_token,
            fee,
            nonce,
            None,
            eth_signature,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if !tx.check_correctness() {
            failure::bail!("Transfer is incorrect, check amounts");
        }
        Ok(tx)
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

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(&self.account.as_bytes());
        out.extend_from_slice(&self.new_pk_hash.data);
        out.extend_from_slice(&self.fee_token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    pub fn get_eth_signed_data(&self) -> Result<Vec<u8>, failure::Error> {
        // Fee data is not included into ETH signature input, since it would require
        // to either have more chunks in pubdata (if fee amount is unpacked), unpack
        // fee on contract (if fee amount is packed), or display non human-readable
        // amount in message (if fee amount is packed and is not unpacked on contract).
        // Either of these options is either non user-friendly or increase cost of
        // operation. Instead, fee data is signed via zkSync signature, which is essentially
        // free. This signature will be verified in the circuit.

        const CHANGE_PUBKEY_SIGNATURE_LEN: usize = 152;
        let mut eth_signed_msg = Vec::with_capacity(CHANGE_PUBKEY_SIGNATURE_LEN);
        eth_signed_msg.extend_from_slice(b"Register zkSync pubkey:\n\n");
        eth_signed_msg.extend_from_slice(
            format!(
                "{pubkey}\n\
                 nonce: 0x{nonce}\n\
                 account id: 0x{account_id}\
                 \n\n",
                pubkey = hex::encode(&self.new_pk_hash.data).to_ascii_lowercase(),
                nonce = hex::encode(&self.nonce.to_be_bytes()).to_ascii_lowercase(),
                account_id = hex::encode(&self.account_id.to_be_bytes()).to_ascii_lowercase()
            )
            .as_bytes(),
        );
        eth_signed_msg.extend_from_slice(b"Only sign this message for a trusted client!");
        ensure!(
            eth_signed_msg.len() == CHANGE_PUBKEY_SIGNATURE_LEN,
            "Change pubkey signed message len is too big: {}, expected: {}",
            eth_signed_msg.len(),
            CHANGE_PUBKEY_SIGNATURE_LEN
        );
        Ok(eth_signed_msg)
    }

    pub fn verify_eth_signature(&self) -> Option<Address> {
        self.eth_signature.as_ref().and_then(|sign| {
            self.get_eth_signed_data()
                .ok()
                .and_then(|msg| sign.signature_recover_signer(&msg).ok())
        })
    }

    pub fn check_correctness(&self) -> bool {
        (self.eth_signature.is_none() || self.verify_eth_signature() == Some(self.account))
            && self.verify_signature() == Some(self.new_pk_hash.clone())
            && self.account_id <= max_account_id()
            && self.fee_token <= max_token_id()
            && is_fee_amount_packable(&self.fee)
    }
}
