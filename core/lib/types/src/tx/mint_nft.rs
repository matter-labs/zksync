use serde::{Deserialize, Serialize};

use num::{BigUint, Zero};

use zksync_crypto::{
    convert::FeConvert,
    franklin_crypto::bellman::pairing::bn256::{Bn256, Fr},
    params::{max_account_id, max_processable_token, CURRENT_TX_VERSION},
    rescue_poseidon::rescue_hash,
    PrivateKey,
};

use zksync_utils::{format_units, BigUintSerdeAsRadix10Str};

use crate::tx::error::TransactionSignatureError;
use crate::tx::version::TxVersion;
use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    tx::{TxSignature, VerifiedSignatureCache},
    AccountId, Address, Nonce, PubKeyHash, TokenId, H256,
};

/// `MintNFT` transaction performs NFT minting for the recipient.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MintNFT {
    /// Id of nft creator
    pub creator_id: AccountId,
    /// Address of nft creator
    pub creator_address: Address,
    /// Hash of data in nft token
    pub content_hash: H256,
    /// Recipient account
    pub recipient: Address,
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    pub fee: BigUint,
    /// Token that will be used for fee.
    #[serde(default)]
    pub fee_token: TokenId,
    /// Current account nonce.
    pub nonce: Nonce,
    /// Transaction zkSync signature.
    pub signature: TxSignature,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
}

impl MintNFT {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 9;

    /// Creates transaction from all the required fields.
    ///
    /// While `signature` field is mandatory for new transactions, it may be `None`
    /// in some cases (e.g. when restoring the network state from the L1 contract data).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        creator_id: AccountId,
        creator_address: Address,
        content_hash: H256,
        recipient: Address,
        fee: BigUint,
        fee_token: TokenId,
        nonce: Nonce,
        signature: Option<TxSignature>,
    ) -> Self {
        let mut tx = Self {
            creator_id,
            creator_address,
            content_hash,
            recipient,
            fee,
            fee_token,
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
        creator_id: AccountId,
        creator_address: Address,
        content_hash: H256,
        recipient: Address,
        fee: BigUint,
        fee_token: TokenId,
        nonce: Nonce,
        private_key: &PrivateKey,
    ) -> Result<Self, TransactionSignatureError> {
        let mut tx = Self::new(
            creator_id,
            creator_address,
            content_hash,
            recipient,
            fee,
            fee_token,
            nonce,
            None,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if !tx.check_correctness() {
            return Err(TransactionSignatureError);
        }
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
        out.extend_from_slice(&self.creator_id.to_be_bytes());
        out.extend_from_slice(&self.creator_address.as_bytes());
        out.extend_from_slice(&self.content_hash.as_bytes());
        out.extend_from_slice(&self.recipient.as_bytes());
        out.extend_from_slice(&self.fee_token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    /// Verifies the transaction correctness:
    ///
    /// - `creator_account_id` field must be within supported range.
    /// - `fee_token` field must be within supported range.
    /// - `fee` field must represent a packable value.
    pub fn check_correctness(&mut self) -> bool {
        let mut valid = self.fee <= BigUint::from(u128::MAX)
            && is_fee_amount_packable(&self.fee)
            && self.creator_id <= max_account_id()
            && self.fee_token <= max_processable_token();
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
                .verify_musig(&self.get_bytes())
                .map(|pub_key| (PubKeyHash::from_pubkey(&pub_key), TxVersion::V1))
        }
    }

    /// Get the first part of the message we expect to be signed by Ethereum account key.
    /// The only difference is the missing `nonce` since it's added at the end of the transactions
    /// batch message.
    pub fn get_ethereum_sign_message_part(&self, token_symbol: &str, decimals: u8) -> String {
        let mut message = format!(
            "MintNFT {content:?} for: {recipient:?}",
            content = self.content_hash,
            recipient = self.recipient
        );
        if !self.fee.is_zero() {
            message.push('\n');
            message.push_str(
                format!(
                    "Fee: {fee} {token}",
                    fee = format_units(self.fee.clone(), decimals),
                    token = token_symbol
                )
                .as_str(),
            );
        }
        message
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

pub fn calculate_token_address(data: &[u8]) -> Address {
    Address::from_slice(&data[12..])
}

pub fn calculate_token_data(data: &[u8]) -> BigUint {
    BigUint::from_bytes_be(&data[16..])
}

pub fn calculate_token_hash(creator_id: AccountId, serial_id: u32, content_hash: H256) -> Vec<u8> {
    let mut lhs_be_bits = vec![];
    lhs_be_bits.extend_from_slice(&creator_id.0.to_be_bytes());
    lhs_be_bits.extend_from_slice(&serial_id.to_be_bytes());
    lhs_be_bits.extend_from_slice(&content_hash.as_bytes()[..16]);
    let lhs_fr = Fr::from_hex(&format!("0x{}", hex::encode(&lhs_be_bits))).expect("lhs as Fr");

    let mut rhs_be_bits = vec![];
    rhs_be_bits.extend_from_slice(&content_hash.as_bytes()[16..]);
    let rhs_fr = Fr::from_hex(&format!("0x{}", hex::encode(&rhs_be_bits))).expect("rhs as Fr");

    let hash_result = rescue_hash::<Bn256, 2>(&[lhs_fr, rhs_fr]);
    hash_result[0].to_bytes()
}
