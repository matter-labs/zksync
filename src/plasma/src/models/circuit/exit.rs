use ff::{BitIterator, PrimeField};

use sapling_crypto::jubjub::JubjubEngine;

use crate::models::params;

#[derive(Clone)]
pub struct ExitRequest<E: JubjubEngine> {
    pub from: E::Fr,
    pub amount: E::Fr,
}

impl<E: JubjubEngine> ExitRequest<E> {
    pub fn public_data_into_bits(&self) -> Vec<bool> {
        // fields are
        // - from
        // - amount
        let mut from: Vec<bool> = BitIterator::new(self.from.clone().into_repr()).collect();
        from.reverse();
        from.truncate(params::BALANCE_TREE_DEPTH);
        // reverse again to have BE as in Ethereum native types
        from.reverse();

        let mut amount: Vec<bool> = BitIterator::new(self.amount.clone().into_repr()).collect();
        amount.reverse();
        amount.truncate(params::BALANCE_BIT_WIDTH);
        // reverse again to have BE as in Ethereum native types
        amount.reverse();

        let mut packed: Vec<bool> = vec![];
        packed.extend(from.into_iter());
        packed.extend(amount.into_iter());

        packed
    }
}
