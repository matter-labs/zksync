use sapling_crypto::jubjub::JubjubEngine;
use sapling_crypto::eddsa::PublicKey;

#[derive(Clone)]
pub struct DepositRequest<E: JubjubEngine> {
    pub into:       E::Fr,
    pub amount:     E::Fr,
    pub pub_x:      E::Fr,
    pub pub_y:      E::Fr,
}