use super::{Nonce, TokenId};

use crate::node::{
    is_fee_amount_packable, is_token_amount_packable, pack_fee_amount, pack_token_amount, CloseOp,
    TransferOp, WithdrawOp,
};
use bigdecimal::BigDecimal;
use crypto::{digest::Digest, sha2::Sha256};

use super::account::AccountAddress;
use super::Engine;
use crate::params::JUBJUB_PARAMS;
use crate::primitives::{big_decimal_to_u128, pedersen_hash_tx_msg, u128_to_bigdecimal};
use ethsign::Signature as ETHSignature;
use failure::{ensure, format_err};
use crate::franklin_crypto::bellman::pairing::ff::{PrimeField, PrimeFieldRepr};
use crate::franklin_crypto::alt_babyjubjub::fs::FsRepr;
use crate::franklin_crypto::alt_babyjubjub::JubjubEngine;
use crate::franklin_crypto::alt_babyjubjub::{edwards, AltJubjubBn256};
use crate::franklin_crypto::eddsa::{PrivateKey, PublicKey, Signature};
use crate::franklin_crypto::jubjub::FixedGenerators;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::convert::TryInto;
use std::str::FromStr;
use web3::types::Address;

#[derive(Clone, PartialEq, Default, Eq, Hash, PartialOrd, Ord)]
pub struct TxHash {
    data: [u8; 32],
}

impl AsRef<[u8]> for TxHash {
    fn as_ref(&self) -> &[u8] {
        &self.data
    }
}

impl ToString for TxHash {
    fn to_string(&self) -> String {
        format!("sync-tx:{}", hex::encode(&self.data))
    }
}

impl FromStr for TxHash {
    type Err = failure::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ensure!(
            s.starts_with("sync-tx:"),
            "TxHash should start with sync-tx:"
        );
        let bytes = hex::decode(&s[8..])?;
        ensure!(bytes.len() == 32, "Size mismatch");
        Ok(TxHash {
            data: bytes.as_slice().try_into().unwrap(),
        })
    }
}

impl Serialize for TxHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for TxHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            Self::from_str(&string).map_err(|err| Error::custom(err.to_string()))
        })
    }
}

/// Signed by user.

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TxType {
    Transfer,
    Withdraw,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
            AccountAddress::from_pubkey(&pub_key) == self.from
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Withdraw {
    pub account: AccountAddress,
    pub eth_address: Address,
    pub token: TokenId,
    pub amount: BigDecimal,
    pub fee: BigDecimal,
    pub nonce: Nonce,
    pub signature: TxSignature,
}

impl Withdraw {
    const TX_TYPE: u8 = 3;

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
            AccountAddress::from_pubkey(&pub_key) == self.account
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

    pub fn verify_signature(&self) -> bool {
        if let Some(pub_key) = self.signature.verify_musig_pedersen(&self.get_bytes()) {
            AccountAddress::from_pubkey(&pub_key) == self.account
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
    pub fn hash(&self) -> TxHash {
        let bytes = match self {
            FranklinTx::Transfer(tx) => tx.get_bytes(),
            FranklinTx::Withdraw(tx) => tx.get_bytes(),
            FranklinTx::Close(tx) => tx.get_bytes(),
        };

        let mut hasher = Sha256::new();
        hasher.input(&bytes);
        let mut out = [0u8; 32];
        hasher.result(&mut out);
        TxHash { data: out }
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

    pub fn min_chunks(&self) -> usize {
        match self {
            FranklinTx::Transfer(_) => TransferOp::CHUNKS,
            FranklinTx::Withdraw(_) => WithdrawOp::CHUNKS,
            FranklinTx::Close(_) => CloseOp::CHUNKS,
        }
    }

    pub fn is_withdraw(&self) -> bool {
        match self {
            FranklinTx::Withdraw(_) => true,
            _ => false,
        }
    }

    pub fn is_close(&self) -> bool {
        match self {
            FranklinTx::Close(_) => true,
            _ => false,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TxSignature {
    pub pub_key: PackedPublicKey,
    pub signature: PackedSignature,
}

impl TxSignature {
    pub fn default() -> Self {
        Self {
            pub_key: PackedPublicKey::deserialize_packed(&[0; 32]).unwrap(),
            signature: PackedSignature::deserialize_packed(&[0; 64]).unwrap(),
        }
    }

    pub fn verify_musig_pedersen(&self, msg: &[u8]) -> Option<PublicKey<Engine>> {
        let hashed_msg = pedersen_hash_tx_msg(msg);
        let valid = self.pub_key.0.verify_musig_pedersen(
            &hashed_msg,
            &self.signature.0,
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
            &self.signature.0,
            FixedGenerators::SpendingKeyGenerator,
            &JUBJUB_PARAMS,
        );
        if valid {
            Some(self.pub_key.0.clone())
        } else {
            None
        }
    }

    pub fn sign_musig_pedersen(pk: &PrivateKey<Engine>, msg: &[u8]) -> Self {
        let hashed_msg = pedersen_hash_tx_msg(msg);
        let signature = pk.musig_pedersen_sign(
            &hashed_msg,
            &mut crate::rand::thread_rng(),
            FixedGenerators::SpendingKeyGenerator,
            &JUBJUB_PARAMS,
        );

        Self {
            pub_key: PackedPublicKey(PublicKey::from_private(
                pk,
                FixedGenerators::SpendingKeyGenerator,
                &JUBJUB_PARAMS,
            )),
            signature: PackedSignature(signature),
        }
    }

    pub fn sign_musig_sha256(pk: &PrivateKey<Engine>, msg: &[u8]) -> Self {
        let hashed_msg = pedersen_hash_tx_msg(msg);
        let signature = pk.musig_sha256_sign(
            &hashed_msg,
            &mut crate::rand::thread_rng(),
            FixedGenerators::SpendingKeyGenerator,
            &JUBJUB_PARAMS,
        );

        Self {
            pub_key: PackedPublicKey(PublicKey::from_private(
                pk,
                FixedGenerators::SpendingKeyGenerator,
                &JUBJUB_PARAMS,
            )),
            signature: PackedSignature(signature),
        }
    }
}

impl std::fmt::Debug for TxSignature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> Result<(), std::fmt::Error> {
        let hex_pk = hex::encode(&self.pub_key.serialize_packed().unwrap());
        let hex_sign = hex::encode(&self.signature.serialize_packed().unwrap());
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

/// Struct used for working with ethereum signatures created using eth_sign (using geth, ethers.js, etc)
/// message is serialized as 65 bytes long `0x` prefixed string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackedEthSignature(pub ETHSignature);

impl PackedEthSignature {
    pub fn serialize_packed(&self) -> [u8; 65] {
        let mut result = [0u8; 65];
        result[0..32].copy_from_slice(&self.0.r);
        result[32..64].copy_from_slice(&self.0.s);
        result[64] = self.0.v;
        result
    }

    pub fn deserialize_packed(bytes: &[u8]) -> Result<Self, failure::Error> {
        ensure!(bytes.len() == 65, "eth signature length should be 65 bytes");
        Ok(PackedEthSignature(ETHSignature {
            r: bytes[0..32].try_into().unwrap(),
            s: bytes[32..64].try_into().unwrap(),
            v: bytes[64],
        }))
    }

    /// Checks signature and returns ethereum address of the signer.
    /// message should be the same message that was passed to `eth.sign`(or similar) method
    /// as argument (no hashing and prefixes required).
    pub fn signature_recover_signer(&self, msg: &[u8]) -> Result<Address, failure::Error> {
        let mut signature = self.0.clone();
        // workaround around ethsign library limitations
        signature.v = signature
            .v
            .checked_sub(27)
            .ok_or_else(|| format_err!("signature v should be 27 + recovery_id"))?;

        let prefix = format!("\x19Ethereum Signed Message:\n{}", msg.len());
        let mut bytes = Vec::new();
        bytes.extend_from_slice(prefix.as_bytes());
        bytes.extend_from_slice(msg);
        let final_message = tiny_keccak::keccak256(&bytes);

        let pk = signature.recover(&final_message)?;
        Ok(Address::from_slice(pk.address()))
    }
}

impl Serialize for PackedEthSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let packed_signature = self.serialize_packed();
        serializer.serialize_str(&format!("0x{}", &hex::encode(&packed_signature[..])))
    }
}

impl<'de> Deserialize<'de> for PackedEthSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            if !string.starts_with("0x") {
                return Err(Error::custom("Packed eth signature should start with 0x"));
            }
            let bytes = hex::decode(&string[2..]).map_err(|e| Error::custom(e.to_string()))?;
            PackedEthSignature::deserialize_packed(&bytes).map_err(|e| Error::custom(e.to_string()))
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::rand::{Rng, SeedableRng, XorShiftRng};

    fn gen_pk_and_msg() -> (PrivateKey<Engine>, Vec<Vec<u8>>) {
        let mut rng = XorShiftRng::from_seed([1, 2, 3, 4]);

        let pk = PrivateKey(rng.gen());

        let mut messages = Vec::new();
        messages.push(Vec::<u8>::new());
        messages.push(b"hello world".to_vec());

        (pk, messages)
    }

    #[test]
    fn test_musig_pedersen_signing_verification() {
        let (pk, messages) = gen_pk_and_msg();

        for msg in &messages {
            let signature = TxSignature::sign_musig_pedersen(&pk, msg);

            if let Some(sign_pub_key) = signature.verify_musig_pedersen(msg) {
                let pub_key = PublicKey::from_private(
                    &pk,
                    FixedGenerators::SpendingKeyGenerator,
                    &JUBJUB_PARAMS,
                );
                assert!(
                    sign_pub_key.0.eq(&pub_key.0),
                    "Signature pub key is wrong, msg: {}",
                    hex::encode(&msg)
                );
            } else {
                panic!("Signature is incorrect, msg: {}", hex::encode(&msg));
            }
        }
    }

    #[test]
    fn test_musig_sha256_signing_verification() {
        let (pk, messages) = gen_pk_and_msg();

        for msg in &messages {
            let signature = TxSignature::sign_musig_sha256(&pk, msg);

            if let Some(sign_pub_key) = signature.verify_musig_sha256(msg) {
                let pub_key = PublicKey::from_private(
                    &pk,
                    FixedGenerators::SpendingKeyGenerator,
                    &JUBJUB_PARAMS,
                );
                assert!(
                    sign_pub_key.0.eq(&pub_key.0),
                    "Signature pub key is wrong, msg: {}",
                    hex::encode(&msg)
                );
            } else {
                panic!("Signature is incorrect, msg: {}", hex::encode(&msg));
            }
        }
    }

    #[test]
    fn test_ethereum_signature_with_serialization() {
        let address: Address = "52312AD6f01657413b2eaE9287f6B9ADaD93D5FE".parse().unwrap();
        let message = "hello world";

        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct TestSignatureSerialize {
            signature: PackedEthSignature,
        }

        // signature calculated using ethers.js signer
        let test_signature_serialize = "{ \"signature\": \"0x111ea2824732851dd0893eaa5873597ba38ed08b69f6d8a0d7f5da810335566403d05281b1f56d12ca653e32eb7d67b76814b0cc8b0da2d7ad2c862d575329951b\"}";

        // test serialization
        let deserialized_signature: TestSignatureSerialize =
            serde_json::from_str(test_signature_serialize).expect("signature deserialize");
        let signature_after_roundtrip: TestSignatureSerialize = serde_json::from_str(
            &serde_json::to_string(&deserialized_signature).expect("signature serialize roundtrip"),
        )
        .expect("signature deserialize roundtrip");
        assert_eq!(
            deserialized_signature, signature_after_roundtrip,
            "signature serialize-deserialize roundtrip"
        );

        let recovered_address = deserialized_signature
            .signature
            .signature_recover_signer(message.as_bytes())
            .expect("signature verification");

        assert_eq!(address, recovered_address, "recovered address mismatch");
    }

    #[test]
    fn test_ethereum_signature_examples() {
        // signatures created using geth
        // e.g. in geth console: eth.sign(eth.accounts[0], "0x")
        let examples = vec![
            ("0x8a91dc2d28b689474298d91899f0c1baf62cb85b", "0xdead", "0x13c34c76ffb42d97da67ddc5d275e92d758d1b48b5ee4b3bacd800cbeec3baff043a5ee63fea55485e1ee5d6f8b088daabd095f2ebbdc80a33806528b44bfccc1c"),
            // empty message
            ("0x8a91dc2d28b689474298d91899f0c1baf62cb85b", "0x", "0xd98f51c2ee0fd589e421348002dffec5d1b38e5bef9a41a699030456dc39298d12698158dc2a814b5f9ac6d433009dec87484a4579107be3f8f33907e92938291b"),
            // this example has v = 28, unlike others
            ("0x8a91dc2d28b689474298d91899f0c1baf62cb85b", "0x14", "0xd288b623af654c9d805e132812edf09ce244040376ca49112e91d437ecceed7c518690d4ae14149cd533f1ca4f081e6d2252c980fccc63de4d6bb818f1b668921c"),
            ];

        for (address, msg, signature) in examples {
            println!("addr: {}, msg: {}, sign: {}", address, msg, signature);
            let address = address[2..].parse::<Address>().unwrap();
            let msg = hex::decode(&msg[2..]).unwrap();
            let signature =
                PackedEthSignature::deserialize_packed(&hex::decode(&signature[2..]).unwrap())
                    .expect("signature deserialize");
            let signer_address = signature
                .signature_recover_signer(&msg)
                .expect("signature verification");
            assert_eq!(address, signer_address, "signer address mismatch");
        }
    }
}
