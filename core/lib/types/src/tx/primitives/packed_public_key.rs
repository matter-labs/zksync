use anyhow::{ensure, format_err};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use zksync_crypto::franklin_crypto::{
    alt_babyjubjub::{edwards, AltJubjubBn256},
    eddsa::PublicKey,
};
use zksync_crypto::params::JUBJUB_PARAMS;
use zksync_crypto::Engine;

#[derive(Clone)]
pub struct PackedPublicKey(pub PublicKey<Engine>);

impl PackedPublicKey {
    pub fn serialize_packed(&self) -> std::io::Result<Vec<u8>> {
        let mut packed_point = [0u8; 32];
        (self.0).0.write(packed_point.as_mut())?;
        Ok(packed_point.to_vec())
    }

    pub fn deserialize_packed(bytes: &[u8]) -> Result<Self, anyhow::Error> {
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
        let packed_point = self.serialize_packed().map_err(serde::ser::Error::custom)?;
        serializer.serialize_str(&hex::encode(packed_point))
    }
}

impl<'de> Deserialize<'de> for PackedPublicKey {
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
