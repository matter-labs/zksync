use std::collections::{hash_map, HashMap};

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::plasma_state::{Account, State, Block};

pub struct StateImpl<E: JubjubEngine> {
    
    // current state of accounts
    accounts:       HashMap<usize, Account<E>>,

    // current block number
    block_number:   u32,

    // current root hash
    root_hash:      E::Fr,
}

impl<'a, E: JubjubEngine> State<'a, E> for StateImpl<E> {
    
    fn accounts_iter(&'a self) -> hash_map::Iter<'a, usize, Account<E>> {
        self.accounts.iter()
    }

    fn block_number(&self) -> u32 {
        self.block_number
    }

    fn root_hash(&'a self) -> &'a E::Fr {
        &self.root_hash
    }
}