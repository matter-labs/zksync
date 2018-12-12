use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::tx::Tx;

pub struct Block<E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<Tx<E>>,
    pub new_root_hash:  E::Fr,
}
