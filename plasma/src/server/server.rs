use std::collections::{hash_map, HashMap};

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::plasma_state::{Account, State, Block};

pub struct StateImpl<'a, E: JubjubEngine> {
    
    // current state of accounts
    accounts:       &'a HashMap<u32, Account<E>>,

    // current block number
    block_number:   u32,

    // current root hash
    root_hash:      &'a E::Fr,
}

impl<'a, E: JubjubEngine> State<'a, E> for StateImpl<'a, E> {

    fn get_accounts(&'a self) -> Vec<(u32, Account<E>)> {
        let capacity = self.accounts.capacity();
        let mut accs = Vec::with_capacity(capacity);

        for (k, v) in self.accounts.iter() {
            let account_number = *k;
            let account_info = v.clone();
            accs.push((account_number, account_info));
        }

        accs
    }
    
    fn accounts_iter(&'a self) -> hash_map::Iter<'a, u32, Account<E>> {
        self.accounts.iter()
    }

    fn block_number(&self) -> u32 {
        self.block_number
    }

    fn root_hash (&'a self) -> E::Fr {
        self.root_hash.clone()
    }
}