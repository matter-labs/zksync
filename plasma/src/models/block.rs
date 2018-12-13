use ff::Field;
use sapling_crypto::jubjub::JubjubEngine;

pub struct Block<T: Sized, E: JubjubEngine> {
    pub block_number:   u32,
    pub transactions:   Vec<T>,
    pub new_root_hash:  E::Fr,
}

impl<T: Sized, E: JubjubEngine> Block<T, E> {

    pub fn with(transactions: Vec<T>) -> Self {
        Self{
            block_number:   0,
            transactions,
            new_root_hash:  E::Fr::zero(),
        }
    }

}