use sapling_crypto::jubjub::JubjubEngine;
use sapling_crypto::eddsa::PublicKey;

#[derive(Clone)]
pub struct ExitRequest<E: JubjubEngine> {
    pub from:   E::Fr,
    pub amount: E::Fr,
}
