use super::{Nonce, TokenId};

use super::operations::{
    CloseOp, TransferOp, TransferToNewOp, WithdrawOp, ACCOUNT_ID_BYTES_LENGTH,
    ETH_ADDR_BYTES_LENGTH, FEE_BYTES_LENGTH, FULL_AMOUNT_BYTES_LENGTH, PACKED_AMOUNT_BYTES_LENGTH,
    TOKEN_BYTES_LENGTH,
};
use crate::node::{
    is_fee_amount_packable, is_token_amount_packable, pack_fee_amount, pack_token_amount,
    unpack_fee_amount, unpack_token_amount,
};
use crate::params::FR_ADDRESS_LEN;
use crate::primitives::bytes_slice_to_uint16;
use bigdecimal::BigDecimal;
use crypto::{digest::Digest, sha2::Sha256};

use super::account::AccountAddress;
use super::Engine;
use crate::params::JUBJUB_PARAMS;
use crate::primitives::{
    big_decimal_to_u128, bytes_slice_to_uint128, pedersen_hash_tx_msg, u128_to_bigdecimal,
};
use failure::{ensure, format_err};
use ff::{PrimeField, PrimeFieldRepr};
use franklin_crypto::alt_babyjubjub::fs::FsRepr;
use franklin_crypto::alt_babyjubjub::JubjubEngine;
use franklin_crypto::alt_babyjubjub::{edwards, AltJubjubBn256};
use franklin_crypto::eddsa::{PublicKey, Signature};
use franklin_crypto::jubjub::FixedGenerators;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use web3::types::Address;

/// Signed by user.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TxType {
    Transfer,
    Withdraw,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transfer {
    pub from: AccountAddress,
    pub to: AccountAddress,
    pub token: TokenId,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub nonce: Nonce,
    pub signature: TxSignature,
}

impl Transfer {
    const TX_TYPE: u8 = 5;

    pub fn from_transfer_to_new_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != TransferToNewOp::OP_LENGTH {
            return None;
        }
        let token_id_pre_length = ACCOUNT_ID_BYTES_LENGTH;
        let amount_pre_length = token_id_pre_length + TOKEN_BYTES_LENGTH;
        let to_pre_length = amount_pre_length + PACKED_AMOUNT_BYTES_LENGTH;
        let fee_pre_length = to_pre_length + FR_ADDRESS_LEN + ACCOUNT_ID_BYTES_LENGTH;

        Some(Self {
            from: AccountAddress::zero(), // From pubdata its unknown
            to: AccountAddress::from_bytes(&bytes[to_pre_length..to_pre_length + FR_ADDRESS_LEN])
                .ok()?,
            token: bytes_slice_to_uint16(
                &bytes[token_id_pre_length..token_id_pre_length + TOKEN_BYTES_LENGTH],
            )?,
            amount: unpack_token_amount(
                &bytes[amount_pre_length..amount_pre_length + PACKED_AMOUNT_BYTES_LENGTH],
            )?,
            fee: unpack_fee_amount(&bytes[fee_pre_length..fee_pre_length + FEE_BYTES_LENGTH])?,
            nonce: 0,                          // From pubdata its unknown
            signature: TxSignature::default(), // From pubdata its unknown
        })
    }

    pub fn from_transfer_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != TransferOp::OP_LENGTH {
            return None;
        }
        let token_id_pre_length = ACCOUNT_ID_BYTES_LENGTH;
        let amount_pre_length = token_id_pre_length + TOKEN_BYTES_LENGTH + ACCOUNT_ID_BYTES_LENGTH;
        let fee_pre_length = amount_pre_length + PACKED_AMOUNT_BYTES_LENGTH;
        Some(Self {
            from: AccountAddress::zero(), // From pubdata its unknown
            to: AccountAddress::zero(),   // From pubdata its unknown
            token: bytes_slice_to_uint16(
                &bytes[token_id_pre_length..token_id_pre_length + TOKEN_BYTES_LENGTH],
            )?,
            amount: unpack_token_amount(
                &bytes[amount_pre_length..amount_pre_length + PACKED_AMOUNT_BYTES_LENGTH],
            )?,
            fee: unpack_fee_amount(&bytes[fee_pre_length..fee_pre_length + FEE_BYTES_LENGTH])?,
            nonce: 0,                          // From pubdata its unknown
            signature: TxSignature::default(), // From pubdata its unknown
        })
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.from.data);
        out.extend_from_slice(&self.to.data);
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&pack_token_amount(&self.amount));
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    pub fn check_correctness(&self) -> bool {
        self.from != self.to
            && is_token_amount_packable(&self.amount)
            && is_fee_amount_packable(&self.fee)
    }

    pub fn verify_signature(&self) -> bool {
        if let Some(pub_key) = self.signature.verify_musig_pedersen(&self.get_bytes()) {
            AccountAddress::from_pubkey(pub_key) == self.from
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Withdraw {
    // TODO: derrive account address from signature
    pub account: AccountAddress,
    pub eth_address: Address,
    pub token: TokenId,
    /// None -> withdraw all
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub nonce: Nonce,
    pub signature: TxSignature,
}

impl Withdraw {
    const TX_TYPE: u8 = 3;

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != WithdrawOp::OP_LENGTH {
            return None;
        }
        let token_id_pre_length = ACCOUNT_ID_BYTES_LENGTH;
        let amount_pre_length = token_id_pre_length + TOKEN_BYTES_LENGTH;
        let fee_pre_length = amount_pre_length + FULL_AMOUNT_BYTES_LENGTH;
        let eth_address_pre_length = fee_pre_length + FEE_BYTES_LENGTH;

        Some(Self {
            account: AccountAddress::zero(), // From pubdata its unknown
            eth_address: Address::from_slice(
                &bytes[eth_address_pre_length..eth_address_pre_length + ETH_ADDR_BYTES_LENGTH],
            ),
            token: bytes_slice_to_uint16(
                &bytes[token_id_pre_length..token_id_pre_length + TOKEN_BYTES_LENGTH],
            )?,
            amount: u128_to_bigdecimal(bytes_slice_to_uint128(
                &bytes[amount_pre_length..amount_pre_length + FULL_AMOUNT_BYTES_LENGTH],
            )?),
            fee: unpack_fee_amount(&bytes[fee_pre_length..fee_pre_length + FEE_BYTES_LENGTH])?,
            nonce: 0,                          // From pubdata its unknown
            signature: TxSignature::default(), // From pubdata its unknown
        })
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account.data);
        out.extend_from_slice(self.eth_address.as_bytes());
        out.extend_from_slice(&self.token.to_be_bytes());
        out.extend_from_slice(&big_decimal_to_u128(&self.amount).to_be_bytes());
        out.extend_from_slice(&pack_fee_amount(&self.fee));
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    pub fn check_correctness(&self) -> bool {
        is_fee_amount_packable(&self.fee) && self.amount <= u128_to_bigdecimal(u128::max_value())
    }

    pub fn verify_signature(&self) -> bool {
        if let Some(pub_key) = self.signature.verify_musig_pedersen(&self.get_bytes()) {
            AccountAddress::from_pubkey(pub_key) == self.account
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Close {
    pub account: AccountAddress,
    pub nonce: Nonce,
    pub signature: TxSignature,
}

impl Close {
    const TX_TYPE: u8 = 4;

    pub fn get_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(&[Self::TX_TYPE]);
        out.extend_from_slice(&self.account.data);
        out.extend_from_slice(&self.nonce.to_be_bytes());
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != CloseOp::OP_LENGTH {
            return None;
        }
        Some(Self {
            account: AccountAddress::zero(),   // From pubdata its unknown
            nonce: 0,                          // From pubdata its unknown
            signature: TxSignature::default(), // From pubdata its unknown
        })
    }

    pub fn verify_signature(&self) -> bool {
        if let Some(pub_key) = self.signature.verify_musig_pedersen(&self.get_bytes()) {
            AccountAddress::from_pubkey(pub_key) == self.account
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FranklinTx {
    Transfer(Transfer),
    Withdraw(Withdraw),
    Close(Close),
}

impl FranklinTx {
    pub fn hash(&self) -> Vec<u8> {
        let bytes = match self {
            FranklinTx::Transfer(tx) => tx.get_bytes(),
            FranklinTx::Withdraw(tx) => tx.get_bytes(),
            FranklinTx::Close(tx) => tx.get_bytes(),
        };

        let mut hasher = Sha256::new();
        hasher.input(&bytes);
        let mut out = vec![0u8; 32];
        hasher.result(&mut out);
        out
    }

    pub fn account(&self) -> AccountAddress {
        match self {
            FranklinTx::Transfer(tx) => tx.from.clone(),
            FranklinTx::Withdraw(tx) => tx.account.clone(),
            FranklinTx::Close(tx) => tx.account.clone(),
        }
    }

    pub fn nonce(&self) -> Nonce {
        match self {
            FranklinTx::Transfer(tx) => tx.nonce,
            FranklinTx::Withdraw(tx) => tx.nonce,
            FranklinTx::Close(tx) => tx.nonce,
        }
    }

    pub fn check_correctness(&self) -> bool {
        match self {
            FranklinTx::Transfer(tx) => tx.check_correctness(),
            FranklinTx::Withdraw(tx) => tx.check_correctness(),
            FranklinTx::Close(_) => true,
        }
    }

    pub fn check_signature(&self) -> bool {
        match self {
            FranklinTx::Transfer(tx) => tx.verify_signature(),
            FranklinTx::Withdraw(tx) => tx.verify_signature(),
            FranklinTx::Close(tx) => tx.verify_signature(),
        }
    }

    pub fn get_bytes(&self) -> Vec<u8> {
        match self {
            FranklinTx::Transfer(tx) => tx.get_bytes(),
            FranklinTx::Withdraw(tx) => tx.get_bytes(),
            FranklinTx::Close(tx) => tx.get_bytes(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TxSignature {
    pub pub_key: PackedPublicKey,
    pub sign: PackedSignature,
}

impl TxSignature {
    pub fn default() -> Self {
        Self {
            pub_key: PackedPublicKey::deserialize_packed(&[0; 32]).unwrap(),
            sign: PackedSignature::deserialize_packed(&[0; 64]).unwrap(),
        }
    }

    pub fn verify_musig_pedersen(&self, msg: &[u8]) -> Option<PublicKey<Engine>> {
        let hashed_msg = pedersen_hash_tx_msg(msg);
        let valid = self.pub_key.0.verify_musig_pedersen(
            &hashed_msg,
            &self.sign.0,
            FixedGenerators::SpendingKeyGenerator,
            &JUBJUB_PARAMS,
        );
        if valid {
            Some(self.pub_key.0.clone())
        } else {
            None
        }
    }

    pub fn verify_musig_sha256(&self, msg: &[u8]) -> Option<PublicKey<Engine>> {
        let hashed_msg = pedersen_hash_tx_msg(msg);
        let valid = self.pub_key.0.verify_musig_sha256(
            &hashed_msg,
            &self.sign.0,
            FixedGenerators::SpendingKeyGenerator,
            &JUBJUB_PARAMS,
        );
        if valid {
            Some(self.pub_key.0.clone())
        } else {
            None
        }
    }
}

impl std::fmt::Debug for TxSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let hex_pk = hex::encode(&self.pub_key.serialize_packed().unwrap());
        let hex_sign = hex::encode(&self.sign.serialize_packed().unwrap());
        write!(f, "{{ pub_key: {}, sign: {} }}", hex_pk, hex_sign)
    }
}

#[derive(Clone)]
pub struct PackedPublicKey(pub PublicKey<Engine>);

impl PackedPublicKey {
    pub fn serialize_packed(&self) -> std::io::Result<Vec<u8>> {
        let mut packed_point = [0u8; 32];
        (self.0).0.write(packed_point.as_mut())?;
        Ok(packed_point.to_vec())
    }

    pub fn deserialize_packed(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(bytes.len() == 32, "PublicKey size mismatch");

        Ok(PackedPublicKey(PublicKey::<Engine>(
            edwards::Point::read(&*bytes, &JUBJUB_PARAMS as &AltJubjubBn256)
                .map_err(|e| format_err!("Failed to restore point: {}", e.to_string()))?,
        )))
    }
}

impl Serialize for PackedPublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;
        let packed_point = self
            .serialize_packed()
            .map_err(|e| Error::custom(e.to_string()))?;

        serializer.serialize_str(&hex::encode(packed_point))
    }
}

impl<'de> Deserialize<'de> for PackedPublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            let bytes = hex::decode(&string).map_err(|e| Error::custom(e.to_string()))?;
            PackedPublicKey::deserialize_packed(&bytes).map_err(|e| Error::custom(e.to_string()))
        })
    }
}

#[derive(Clone)]
pub struct PackedSignature(pub Signature<Engine>);

impl PackedSignature {
    pub fn serialize_packed(&self) -> std::io::Result<Vec<u8>> {
        let mut packed_signature = [0u8; 64];
        let (r_bar, s_bar) = packed_signature.as_mut().split_at_mut(32);

        (self.0).r.write(r_bar)?;
        (self.0).s.into_repr().write_le(s_bar)?;

        Ok(packed_signature.to_vec())
    }

    pub fn deserialize_packed(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(bytes.len() == 64, "Signature size mismatch");
        let (r_bar, s_bar) = bytes.split_at(32);

        let r = edwards::Point::read(r_bar, &JUBJUB_PARAMS as &AltJubjubBn256)
            .map_err(|e| format_err!("Failed to restore R point from R_bar: {}", e.to_string()))?;

        let mut s_repr = FsRepr::default();
        s_repr
            .read_le(s_bar)
            .map_err(|e| format_err!("s read err: {}", e.to_string()))?;

        let s = <Engine as JubjubEngine>::Fs::from_repr(s_repr)
            .map_err(|e| format_err!("Failed to restore s scalar from s_bar: {}", e.to_string()))?;

        Ok(Self(Signature { r, s }))
    }
}

impl Serialize for PackedSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;

        let packed_signature = self
            .serialize_packed()
            .map_err(|e| Error::custom(e.to_string()))?;
        serializer.serialize_str(&hex::encode(&packed_signature))
    }
}

impl<'de> Deserialize<'de> for PackedSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            let bytes = hex::decode(&string).map_err(|e| Error::custom(e.to_string()))?;
            PackedSignature::deserialize_packed(&bytes).map_err(|e| Error::custom(e.to_string()))
        })
    }
}
