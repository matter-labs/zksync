use crate::{
    helpers::{is_fee_amount_packable, pack_fee_amount},
    AccountId, Nonce,
};

use crate::account::PubKeyHash;
use anyhow::ensure;
use num::{BigUint, Zero};
use parity_crypto::Keccak256;
use serde::{Deserialize, Serialize};
use zksync_basic_types::{Address, TokenId, H256};
use zksync_crypto::{
    params::{max_account_id, max_token_id},
    PrivateKey,
};
use zksync_utils::{format_units, BigUintSerdeAsRadix10Str};

use super::{PackedEthSignature, TxSignature, VerifiedSignatureCache};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKeyECDSAData {
    pub eth_signature: PackedEthSignature,
    #[serde(default)]
    pub batch_hash: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKeyCREATE2Data {
    pub creator_address: Address,
    pub salt_arg: H256,
    pub code_hash: H256,
}

impl ChangePubKeyCREATE2Data {
    pub fn get_address(&self, pubkey_hash: &PubKeyHash) -> Address {
        let salt = {
            let mut bytes = Vec::new();
            bytes.extend_from_slice(&pubkey_hash.data);
            bytes.extend_from_slice(self.salt_arg.as_bytes());
            bytes.keccak256()
        };

        let mut bytes = Vec::new();
        bytes.push(0xff);
        bytes.extend_from_slice(self.creator_address.as_bytes());
        bytes.extend_from_slice(&salt);
        bytes.extend_from_slice(self.code_hash.as_bytes());
        Address::from_slice(&bytes.keccak256()[12..])
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ChangePubKeyEthAuthData {
    Onchain,
    ECDSA(ChangePubKeyECDSAData),
    CREATE2(ChangePubKeyCREATE2Data),
}

impl ChangePubKeyEthAuthData {
    pub fn is_ecdsa(&self) -> bool {
        matches!(self, ChangePubKeyEthAuthData::ECDSA(..))
    }

    pub fn is_onchain(&self) -> bool {
        matches!(self, ChangePubKeyEthAuthData::Onchain)
    }

    pub fn get_eth_witness(&self) -> Vec<u8> {
        match self {
            ChangePubKeyEthAuthData::Onchain => Vec::new(),
            ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData { eth_signature, .. }) => {
                let mut bytes = Vec::new();
                bytes.push(0x00);
                bytes.extend_from_slice(&eth_signature.serialize_packed());
                // bytes.extend_from_slice(batch_hash.as_bytes());
                bytes
            }
            ChangePubKeyEthAuthData::CREATE2(ChangePubKeyCREATE2Data {
                creator_address,
                salt_arg,
                code_hash,
            }) => {
                let mut bytes = Vec::new();
                bytes.push(0x01);
                bytes.extend_from_slice(creator_address.as_bytes());
                bytes.extend_from_slice(salt_arg.as_bytes());
                bytes.extend_from_slice(code_hash.as_bytes());
                bytes
            }
        }
    }
}

/// `ChangePubKey` transaction is used to set the owner's public key hash
/// associated with the account.
///
/// Without public key hash set, account is unable to execute any L2 transactions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKey {
    /// zkSync network account ID to apply operation to.
    pub account_id: AccountId,
    /// Address of the account.
    pub account: Address,
    /// Public key hash to set.
    pub new_pk_hash: PubKeyHash,
    /// Token to be used for fee.
    #[serde(default)]
    pub fee_token: TokenId,
    /// Fee for the transaction.
    #[serde(with = "BigUintSerdeAsRadix10Str", default)]
    pub fee: BigUint,
    /// Current account nonce.
    pub nonce: Nonce,
    /// Transaction zkSync signature. Must be signed with the key corresponding to the
    /// `new_pk_hash` value. This signature is required to ensure that `fee_token` and `fee`
    /// fields can't be changed by an attacker.
    #[serde(default)]
    pub signature: TxSignature,
    /// Data needed to check if Ethereum address authorized ChangePubKey operation
    pub eth_auth_data: ChangePubKeyEthAuthData,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
    /// Unix epoch format of the time when the transaction is valid
    /// This fields must be Option<...> because of backward compatibility with first version of ZkSync
    pub valid_from: Option<u32>,
    pub valid_until: Option<u32>,
}

impl ChangePubKey {
    /// Unique identifier of the transaction type in zkSync network.
    pub const TX_TYPE: u8 = 7;

    /// Creates transaction from all the required fields.
    ///
    /// While `signature` field is mandatory for new transactions, it may be `None`
    /// in some cases (e.g. when restoring the network state from the L1 contract data).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        account_id: AccountId,
        account: Address,
        new_pk_hash: PubKeyHash,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        valid_from: u32,
        valid_until: u32,
        signature: Option<TxSignature>,
        eth_signature: Option<PackedEthSignature>,
    ) -> Self {
        // TODO: support CREATE2
        let eth_auth_data = eth_signature
            .map(|eth_signature| {
                ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData {
                    eth_signature,
                    batch_hash: H256::zero(),
                })
            })
            .unwrap_or(ChangePubKeyEthAuthData::Onchain);

        let mut tx = Self {
            account_id,
            account,
            new_pk_hash,
            fee_token,
            fee,
            nonce,
            signature: signature.clone().unwrap_or_default(),
            eth_auth_data,
            cached_signer: VerifiedSignatureCache::NotCached,
            valid_from: Some(valid_from),
            valid_until: Some(valid_until),
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
        account: Address,
        new_pk_hash: PubKeyHash,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        valid_from: u32,
        valid_until: u32,
        eth_signature: Option<PackedEthSignature>,
        private_key: &PrivateKey,
    ) -> Result<Self, anyhow::Error> {
        let mut tx = Self::new(
            account_id,
            account,
            new_pk_hash,
            fee_token,
            fee,
            nonce,
            valid_from,
            valid_until,
            None,
            eth_signature,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        if !tx.check_correctness() {
            anyhow::bail!(crate::tx::TRANSACTION_SIGNATURE_ERROR);
        }
        Ok(tx)
    }

    /// Restores the `PubKeyHash` from the transaction signature.
    pub fn verify_signature(&self) -> Option<PubKeyHash> {
        if let VerifiedSignatureCache::Cached(cached_signer) = &self.cached_signer {
            *cached_signer
        } else if let Some(pub_key) = self.signature.verify_musig(&self.get_bytes()) {
            Some(PubKeyHash::from_pubkey(&pub_key))
        } else {
            None
        }
    }

    /// Encodes the transaction data as the byte sequence according to the zkSync protocol.
    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(&self.account.as_bytes());
        out.extend_from_slice(&self.new_pk_hash.data);
        out.extend_from_slice(&self.fee_token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());

        // We use 64 bytes for timestamps in the signed message
        out.extend_from_slice(&u64::from(self.valid_from.unwrap_or(0)).to_be_bytes());
        out.extend_from_slice(&u64::from(self.valid_until.unwrap_or(u32::MAX)).to_be_bytes());

        out
    }

    /// Provides a message to be signed with the Ethereum private key.
    pub fn get_eth_signed_data(&self) -> Result<Vec<u8>, anyhow::Error> {
        // Fee data is not included into ETH signature input, since it would require
        // to either have more chunks in pubdata (if fee amount is unpacked), unpack
        // fee on contract (if fee amount is packed), or display non human-readable
        // amount in message (if fee amount is packed and is not unpacked on contract).
        // Either of these options is either non user-friendly or increase cost of
        // operation. Instead, fee data is signed via zkSync signature, which is essentially
        // free. This signature will be verified in the circuit.

        const CHANGE_PUBKEY_SIGNATURE_LEN: usize = 60;
        let mut eth_signed_msg = Vec::with_capacity(CHANGE_PUBKEY_SIGNATURE_LEN);
        eth_signed_msg.extend_from_slice(&self.new_pk_hash.data);
        eth_signed_msg.extend_from_slice(&self.nonce.to_be_bytes());
        eth_signed_msg.extend_from_slice(&self.account_id.to_be_bytes());
        // In case this transaction is not part of a batch, we simply append zeros.
        if let ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData { batch_hash, .. }) =
            self.eth_auth_data
        {
            eth_signed_msg.extend_from_slice(batch_hash.as_bytes());
        } else {
            eth_signed_msg.extend_from_slice(H256::default().as_bytes());
        }
        ensure!(
            eth_signed_msg.len() == CHANGE_PUBKEY_SIGNATURE_LEN,
            "Change pubkey signed message does not match in size: {}, expected: {}",
            eth_signed_msg.len(),
            CHANGE_PUBKEY_SIGNATURE_LEN
        );
        Ok(eth_signed_msg)
    }

    pub fn is_eth_auth_data_valid(&self) -> bool {
        match &self.eth_auth_data {
            ChangePubKeyEthAuthData::Onchain => true, // Should query Ethereum to check it
            ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData { eth_signature, .. }) => {
                let recovered_address = self
                    .get_eth_signed_data()
                    .ok()
                    .and_then(|msg| eth_signature.signature_recover_signer(&msg).ok());
                recovered_address == Some(self.account)
            }
            ChangePubKeyEthAuthData::CREATE2(create2_data) => {
                let create2_address = create2_data.get_address(&self.new_pk_hash);
                create2_address == self.account
            }
        }
    }

    /// Verifies the transaction correctness:
    ///
    /// - Ethereum signature (if set) must correspond to the account address.
    /// - zkSync signature must correspond to the `new_pk_hash` field of the transaction.
    /// - `account_id` field must be within supported range.
    /// - `fee_token` field must be within supported range.
    /// - `fee` field must represent a packable value.
    pub fn check_correctness(&self) -> bool {
        self.is_eth_auth_data_valid()
            && self.verify_signature() == Some(self.new_pk_hash)
            && self.account_id <= max_account_id()
            && self.fee_token <= max_token_id()
            && is_fee_amount_packable(&self.fee)
            && self.valid_from.unwrap_or(0) <= self.valid_until.unwrap_or(u32::MAX)
    }

    /// Get part of the message that should be signed with Ethereum account key for the batch of transactions.
    /// The message for single `ChangePubKey` transaction is defined differently. The pattern is:
    ///
    /// Set signing key: {pubKeyHash}
    /// [Fee: {fee} {token}]
    ///
    /// Note that the second line is optional.
    pub fn get_ethereum_sign_message_part(&self, token_symbol: &str, decimals: u8) -> String {
        let mut message = format!(
            "Set signing key: {}",
            hex::encode(&self.new_pk_hash.data).to_ascii_lowercase()
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
}
