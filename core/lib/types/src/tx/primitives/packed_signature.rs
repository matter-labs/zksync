use crate::Engine;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;
use zksync_crypto::{
    franklin_crypto::{
        alt_babyjubjub::{edwards, fs::FsRepr, AltJubjubBn256, JubjubEngine},
        bellman::pairing::ff::{PrimeField, PrimeFieldRepr},
        eddsa::Signature,
    },
    params::JUBJUB_PARAMS,
};

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

    pub fn deserialize_packed(bytes: &[u8]) -> Result<Self, DeserializeError> {
        if bytes.len() != 64 {
            return Err(DeserializeError::IncorrectSignatureLength);
        }
        let (r_bar, s_bar) = bytes.split_at(32);

        let r = edwards::Point::read(r_bar, &JUBJUB_PARAMS as &AltJubjubBn256)
            .map_err(DeserializeError::RestoreRPoint)?;

        let mut s_repr = FsRepr::default();
        s_repr.read_le(s_bar).map_err(DeserializeError::ReadS)?;

        let s =
            <Engine as JubjubEngine>::Fs::from_repr(s_repr).map_err(DeserializeError::RestoreS)?;

        Ok(Self(Signature { r, s }))
    }
}

#[derive(Debug, Error)]
pub enum DeserializeError {
    #[error("Signature length should be 64 bytes")]
    IncorrectSignatureLength,
    #[error("Failed to restore R point from R_bar: {0}")]
    RestoreRPoint(std::io::Error),
    #[error("Cannot read S scalar: {0}")]
    ReadS(std::io::Error),
    #[error("Cannot restore S scalar: {0}")]
    RestoreS(zksync_crypto::ff::PrimeFieldDecodingError),
}

impl Serialize for PackedSignature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let packed_signature = self.serialize_packed().map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&hex::encode(&packed_signature))
    }
}

impl<'de> Deserialize<'de> for PackedSignature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        use serde::de::Error;
        let string = String::deserialize(deserializer)?;
        let bytes = hex::decode(&string).map_err(Error::custom)?;
        Self::deserialize_packed(&bytes).map_err(Error::custom)
    }
}
