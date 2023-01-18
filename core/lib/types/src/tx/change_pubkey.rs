use num::{BigUint, Zero};
use std::fmt::{Display, Formatter};

use parity_crypto::Keccak256;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use zksync_basic_types::{Address, ChainId, TokenId, H256};
use zksync_crypto::{
    params::{max_account_id, max_processable_token, CURRENT_TX_VERSION},
    PrivateKey,
};
use zksync_utils::{format_units, BigUintSerdeAsRadix10Str};

use super::{PackedEthSignature, TimeRange, TxSignature, VerifiedSignatureCache};
use crate::tx::error::{
    ChangePubkeySignedDataError, FEE_AMOUNT_IS_NOT_PACKABLE, INVALID_AUTH_DATA, WRONG_ACCOUNT_ID,
    WRONG_FEE_ERROR, WRONG_SIGNATURE, WRONG_TIME_RANGE, WRONG_TOKEN_FOR_PAYING_FEE,
};

use crate::{
    account::PubKeyHash,
    helpers::{is_fee_amount_packable, pack_fee_amount},
    tokens::ChangePubKeyFeeTypeArg,
    tx::{
        primitives::eip712_signature::{EIP712TypedStructure, Eip712Domain, StructBuilder},
        version::TxVersion,
    },
    AccountId, Nonce, TxFeeTypes,
};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Hash, Eq)]
pub enum ChangePubKeyType {
    Onchain,
    ECDSA,
    CREATE2,
    EIP712,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKeyECDSAData {
    pub eth_signature: PackedEthSignature,
    #[serde(default)]
    pub batch_hash: H256,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangePubKeyEIP712Data {
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
            bytes.extend_from_slice(self.salt_arg.as_bytes());
            bytes.extend_from_slice(&pubkey_hash.data);
            bytes.keccak256()
        };

        let mut bytes = vec![0xff];
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
    EIP712(ChangePubKeyEIP712Data),
}

impl ChangePubKeyEthAuthData {
    pub fn is_ecdsa(&self) -> bool {
        matches!(self, ChangePubKeyEthAuthData::ECDSA(..))
    }

    pub fn is_onchain(&self) -> bool {
        matches!(self, ChangePubKeyEthAuthData::Onchain)
    }

    pub fn is_create2(&self) -> bool {
        matches!(self, ChangePubKeyEthAuthData::CREATE2(..))
    }

    pub fn get_eth_witness(&self) -> Vec<u8> {
        match self {
            ChangePubKeyEthAuthData::Onchain => Vec::new(),
            ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData { eth_signature, .. }) => {
                let mut bytes = vec![0x00];
                bytes.extend_from_slice(&eth_signature.serialize_packed());
                // bytes.extend_from_slice(batch_hash.as_bytes());
                bytes
            }
            ChangePubKeyEthAuthData::CREATE2(ChangePubKeyCREATE2Data {
                creator_address,
                salt_arg,
                code_hash,
            }) => {
                let mut bytes = vec![0x01];
                bytes.extend_from_slice(creator_address.as_bytes());
                bytes.extend_from_slice(salt_arg.as_bytes());
                bytes.extend_from_slice(code_hash.as_bytes());
                bytes
            }
            ChangePubKeyEthAuthData::EIP712(ChangePubKeyEIP712Data { eth_signature, .. }) => {
                let mut bytes = vec![0x4];
                bytes.extend_from_slice(&eth_signature.serialize_packed());
                bytes
            }
        }
    }

    pub fn get_fee_type(&self) -> ChangePubKeyType {
        match self {
            ChangePubKeyEthAuthData::Onchain => ChangePubKeyType::Onchain,
            ChangePubKeyEthAuthData::ECDSA(_) => ChangePubKeyType::ECDSA,
            ChangePubKeyEthAuthData::CREATE2(_) => ChangePubKeyType::CREATE2,
            ChangePubKeyEthAuthData::EIP712(_) => ChangePubKeyType::EIP712,
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
    /// Transaction Ethereum signature. It may be `None` if `ChangePubKey` operation is authorized
    /// onchain, otherwise the message must be signed by the Ethereum private key corresponding
    /// to the account address.
    pub eth_signature: Option<PackedEthSignature>,
    /// Data needed to check if Ethereum address authorized ChangePubKey operation
    pub eth_auth_data: Option<ChangePubKeyEthAuthData>,
    /// Time range when the transaction is valid
    /// This fields must be Option<...> because of backward compatibility with first version of ZkSync
    #[serde(flatten)]
    pub time_range: Option<TimeRange>,
    pub chain_id: Option<ChainId>,
    #[serde(skip)]
    cached_signer: VerifiedSignatureCache,
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
        time_range: TimeRange,
        signature: Option<TxSignature>,
        eth_signature: Option<PackedEthSignature>,
        chain_id: Option<ChainId>,
    ) -> Self {
        // TODO: support CREATE2 (ZKS-452)
        let eth_auth_data = Some(
            eth_signature
                .map(|eth_signature| {
                    ChangePubKeyEthAuthData::EIP712(ChangePubKeyEIP712Data {
                        eth_signature,
                        batch_hash: H256::zero(),
                    })
                })
                .unwrap_or(ChangePubKeyEthAuthData::Onchain),
        );

        let mut tx = Self {
            account_id,
            account,
            new_pk_hash,
            fee_token,
            fee,
            nonce,
            signature: signature.clone().unwrap_or_default(),
            eth_signature: None,
            eth_auth_data,
            cached_signer: VerifiedSignatureCache::NotCached,
            time_range: Some(time_range),
            chain_id,
        };
        if signature.is_some() {
            tx.cached_signer = VerifiedSignatureCache::Cached(tx.verify_signature());
        }
        tx
    }

    /// Creates a signed transaction using private key and
    /// checks the transaction correctness.
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        account_id: AccountId,
        account: Address,
        new_pk_hash: PubKeyHash,
        fee_token: TokenId,
        fee: BigUint,
        nonce: Nonce,
        time_range: TimeRange,
        eth_signature: Option<PackedEthSignature>,
        private_key: &PrivateKey,
        chain_id: Option<ChainId>,
    ) -> Result<Self, TransactionError> {
        let mut tx = Self::new(
            account_id,
            account,
            new_pk_hash,
            fee_token,
            fee,
            nonce,
            time_range,
            None,
            eth_signature,
            chain_id,
        );
        tx.signature = TxSignature::sign_musig(private_key, &tx.get_bytes());
        tx.check_correctness()?;
        Ok(tx)
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

    /// Encodes the transaction data as the byte sequence according to the old zkSync protocol with 2 bytes token.
    pub fn get_old_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account_id.to_be_bytes());
        out.extend_from_slice(self.account.as_bytes());
        out.extend_from_slice(&self.new_pk_hash.data);
        out.extend_from_slice(&(self.fee_token.0 as u16).to_be_bytes());
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
        out.extend_from_slice(self.account.as_bytes());
        out.extend_from_slice(&self.new_pk_hash.data);
        out.extend_from_slice(&self.fee_token.to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        let time_range = self.time_range.unwrap_or_default();
        out.extend_from_slice(&time_range.as_be_bytes());
        out
    }

    /// Provides a message to be signed with the Ethereum private key.
    pub fn get_eth_signed_data(&self) -> Result<Vec<u8>, ChangePubkeySignedDataError> {
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
        match &self.eth_auth_data {
            Some(ChangePubKeyEthAuthData::EIP712(ChangePubKeyEIP712Data {
                batch_hash, ..
            })) => {
                eth_signed_msg.extend_from_slice(batch_hash.as_bytes());
            }
            Some(ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData { batch_hash, .. })) => {
                eth_signed_msg.extend_from_slice(batch_hash.as_bytes());
            }
            _ => {
                eth_signed_msg.extend_from_slice(H256::default().as_bytes());
            }
        }
        if eth_signed_msg.len() != CHANGE_PUBKEY_SIGNATURE_LEN {
            return Err(ChangePubkeySignedDataError::SignedMessageLengthMismatch {
                actual: eth_signed_msg.len(),
                expected: CHANGE_PUBKEY_SIGNATURE_LEN,
            });
        }
        Ok(eth_signed_msg)
    }

    /// Provides an old message to be signed with the Ethereum private key.
    pub fn get_old_eth_signed_data(&self) -> Result<Vec<u8>, ChangePubkeySignedDataError> {
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
                pubkey = hex::encode(self.new_pk_hash.data).to_ascii_lowercase(),
                nonce = hex::encode(self.nonce.to_be_bytes()).to_ascii_lowercase(),
                account_id = hex::encode(self.account_id.to_be_bytes()).to_ascii_lowercase()
            )
            .as_bytes(),
        );
        eth_signed_msg.extend_from_slice(b"Only sign this message for a trusted client!");

        if eth_signed_msg.len() != CHANGE_PUBKEY_SIGNATURE_LEN {
            return Err(ChangePubkeySignedDataError::SignedMessageLengthMismatch {
                actual: eth_signed_msg.len(),
                expected: CHANGE_PUBKEY_SIGNATURE_LEN,
            });
        }
        Ok(eth_signed_msg)
    }

    pub fn is_eth_auth_data_valid(&self) -> bool {
        if let Some(eth_auth_data) = &self.eth_auth_data {
            match eth_auth_data {
                ChangePubKeyEthAuthData::Onchain => true, // Should query Ethereum to check it
                ChangePubKeyEthAuthData::ECDSA(ChangePubKeyECDSAData { eth_signature, .. }) => {
                    let recovered_address = self.get_eth_signed_data().ok().and_then(|msg| {
                        eth_signature
                            .signature_recover_signer_from_raw_message(&msg)
                            .ok()
                    });
                    recovered_address == Some(self.account)
                }
                ChangePubKeyEthAuthData::CREATE2(create2_data) => {
                    let create2_address = create2_data.get_address(&self.new_pk_hash);
                    create2_address == self.account
                }
                ChangePubKeyEthAuthData::EIP712(ChangePubKeyEIP712Data {
                    eth_signature, ..
                }) => {
                    if let Some(chain_id) = self.chain_id {
                        let domain = Eip712Domain::new(chain_id);
                        let data = PackedEthSignature::typed_data_to_signed_bytes(&domain, self);
                        let recovered_address =
                            eth_signature.signature_recover_signer_from_hash(data).ok();
                        recovered_address == Some(self.account)
                    } else {
                        vlog::error!("No chain id for EIP712 data. Tx {:?}", &self);
                        false
                    }
                }
            }
        } else if let Some(old_eth_signature) = &self.eth_signature {
            let recovered_address = self.get_old_eth_signed_data().ok().and_then(|msg| {
                old_eth_signature
                    .signature_recover_signer_from_raw_message(&msg)
                    .ok()
            });
            recovered_address == Some(self.account)
        } else {
            true
        }
    }

    pub fn is_ecdsa(&self) -> bool {
        if let Some(auth_data) = &self.eth_auth_data {
            auth_data.is_ecdsa()
        } else {
            self.eth_signature.is_some()
        }
    }

    pub fn is_onchain(&self) -> bool {
        if let Some(auth_data) = &self.eth_auth_data {
            auth_data.is_onchain()
        } else {
            self.eth_signature.is_none()
        }
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
            hex::encode(self.new_pk_hash.data).to_ascii_lowercase()
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

    pub fn get_change_pubkey_fee_type(&self) -> ChangePubKeyFeeTypeArg {
        if let Some(auth_data) = &self.eth_auth_data {
            ChangePubKeyFeeTypeArg::ContractsV4Version(auth_data.get_fee_type())
        } else {
            ChangePubKeyFeeTypeArg::PreContracts4Version {
                onchain_pubkey_auth: self.eth_signature.is_none(),
            }
        }
    }

    pub fn get_fee_type(&self) -> TxFeeTypes {
        TxFeeTypes::ChangePubKey(self.get_change_pubkey_fee_type())
    }

    /// Helper method to remove cache and test transaction behavior without the signature cache.
    #[doc(hidden)]
    pub fn wipe_signer_cache(&mut self) {
        self.cached_signer = VerifiedSignatureCache::NotCached;
    }

    /// Verifies the transaction correctness:
    ///
    /// - Ethereum signature (if set) must correspond to the account address.
    /// - zkSync signature must correspond to the `new_pk_hash` field of the transaction.
    /// - `account_id` field must be within supported range.
    /// - `fee_token` field must be within supported range.
    /// - `fee` field must represent a packable value.
    pub fn check_correctness(&mut self) -> Result<(), TransactionError> {
        if !self.is_eth_auth_data_valid() {
            return Err(TransactionError::InvalidAuthData);
        }
        if self.fee > BigUint::from(u128::MAX) {
            return Err(TransactionError::WrongFee);
        }
        if self.account_id > max_account_id() {
            return Err(TransactionError::WrongAccountId);
        }

        if self.fee_token > max_processable_token() {
            return Err(TransactionError::WrongFeeToken);
        }
        if !is_fee_amount_packable(&self.fee) {
            return Err(TransactionError::FeeNotPackable);
        }

        if !self
            .time_range
            .map(|r| r.check_correctness())
            .unwrap_or(true)
        {
            return Err(TransactionError::WrongTimeRange);
        }
        let signer = self.verify_signature();
        self.cached_signer = VerifiedSignatureCache::Cached(signer);
        if let Some((pub_key_hash, _)) = &signer {
            if *pub_key_hash != self.new_pk_hash {
                return Err(TransactionError::WrongSignature);
            }
        } else {
            return Err(TransactionError::WrongSignature);
        }
        Ok(())
    }
}

#[derive(Error, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum TransactionError {
    InvalidAuthData,
    WrongFee,
    FeeNotPackable,
    WrongAccountId,
    WrongFeeToken,
    WrongTimeRange,
    WrongSignature,
}

impl Display for TransactionError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let error = match self {
            TransactionError::WrongFee => WRONG_FEE_ERROR,
            TransactionError::FeeNotPackable => FEE_AMOUNT_IS_NOT_PACKABLE,
            TransactionError::WrongAccountId => WRONG_ACCOUNT_ID,
            TransactionError::WrongTimeRange => WRONG_TIME_RANGE,
            TransactionError::WrongSignature => WRONG_SIGNATURE,
            TransactionError::WrongFeeToken => WRONG_TOKEN_FOR_PAYING_FEE,
            TransactionError::InvalidAuthData => INVALID_AUTH_DATA,
        };
        write!(f, "{}", error)
    }
}

impl EIP712TypedStructure for ChangePubKey {
    const TYPE_NAME: &'static str = "ChangePubKey";

    fn build_structure<BUILDER: StructBuilder>(&self, builder: &mut BUILDER) {
        builder.add_member("pubKeyHash", &self.new_pk_hash.data);
        builder.add_member("nonce", &self.nonce.0);
        builder.add_member("accountId", &self.account_id.0);
    }
}
