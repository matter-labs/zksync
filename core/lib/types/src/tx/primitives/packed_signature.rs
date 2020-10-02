use crate::Engine;
use anyhow::{ensure, format_err};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zksync_crypto::franklin_crypto::{
    alt_babyjubjub::{
        fs::FsRepr,
        JubjubEngine, {edwards, AltJubjubBn256},
    },
    bellman::pairing::ff::{PrimeField, PrimeFieldRepr},
    eddsa::Signature,
};
use zksync_crypto::params::JUBJUB_PARAMS;

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

    pub fn deserialize_packed(bytes: &[u8]) -> Result<Self, anyhow::Error> {
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
