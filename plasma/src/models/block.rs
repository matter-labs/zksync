use ff::Field;
use sapling_crypto::jubjub::JubjubEngine;

pub struct Block<T: Sized, E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<T>,
    pub new_root_hash:  E::Fr,
}

impl<T: Sized, E: JubjubEngine> Block<T, E> {

    pub fn empty() -> Self {
        Self{
            block_number:   0,
            transactions: vec![],
            new_root_hash:  E::Fr::zero(),
        }
    }

}