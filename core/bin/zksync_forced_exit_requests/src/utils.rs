use zksync_crypto::ff::PrimeField;
pub use zksync_crypto::franklin_crypto::{eddsa::PrivateKey, jubjub::JubjubEngine};

pub use franklin_crypto::{
    alt_babyjubjub::fs::FsRepr,
    bellman::{pairing::bn256, PrimeFieldRepr},
};

pub type Engine = bn256::Bn256;

pub type Fs = <Engine as JubjubEngine>::Fs;

pub fn read_signing_key(private_key: &[u8]) -> anyhow::Result<PrivateKey<Engine>> {
    let mut fs_repr = FsRepr::default();
    fs_repr.read_be(private_key)?;
    Ok(PrivateKey::<Engine>(
        Fs::from_repr(fs_repr).expect("couldn't read private key from repr"),
    ))
}
