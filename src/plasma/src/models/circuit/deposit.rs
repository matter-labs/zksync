use crate::models::params;
use ff::{BitIterator, PrimeField};
use sapling_crypto::jubjub::JubjubEngine;

#[derive(Clone)]
pub struct DepositRequest<E: JubjubEngine> {
    pub into: E::Fr,
    pub amount: E::Fr,
    pub pub_x: E::Fr,
    pub pub_y: E::Fr,
}

impl<E: JubjubEngine> DepositRequest<E> {
    // this function returns public data in Ethereum compatible format
    pub fn public_data_into_bits(&self) -> Vec<bool> {
        // fields are
        // - into
        // - amount
        // - compressed public key
        let mut into: Vec<bool> = BitIterator::new(self.into.clone().into_repr()).collect();
        into.reverse();
        into.truncate(params::BALANCE_TREE_DEPTH);
        // reverse again to have BE as in Ethereum native types
        into.reverse();

        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().into_repr()).collect();
        amount.reverse();
        amount.truncate(params::BALANCE_BIT_WIDTH);
        // reverse again to have BE as in Ethereum native types
        amount.reverse();

        let mut y_bits: Vec<bool> = BitIterator::new(self.pub_y.clone().into_repr()).collect();
        y_bits.reverse();
        y_bits.truncate(E::Fr::NUM_BITS as usize);
        y_bits.resize(params::FR_BIT_WIDTH - 1, false);

        let mut x_bits: Vec<bool> = BitIterator::new(self.pub_x.clone().into_repr()).collect();
        x_bits.reverse();
        // push sign bit
        y_bits.push(x_bits[0]);
        // reverse again to have BE as in Ethereum native types
        y_bits.reverse();

        let mut packed: Vec<bool> = vec![];
        packed.extend(into.into_iter());
        packed.extend(amount.into_iter());
        packed.extend(y_bits.into_iter());

        packed
    }
}
