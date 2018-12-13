use sapling_crypto::jubjub::JubjubEngine;

pub struct Block<T: Sized, E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<T>,
    pub new_root_hash:  E::Fr,
}