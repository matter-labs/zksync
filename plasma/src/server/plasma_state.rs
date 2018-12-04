use std::collections::HashMap;

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::super::balance_tree;
use super::super::circuit::baby_plasma::TransactionSignature;

pub type Account<E> = balance_tree::Leaf<E>;

pub struct State<E: JubjubEngine> {
    
    // current state of accounts
    accounts:       HashMap<usize, Account<E>>,

    // current block number
    block_number:   u32,

    // current root hash
    root_hash:      E::Fr,
}

impl<'a, E: JubjubEngine> State<E> {
    
    fn accounts(&'a self) -> &'a HashMap<usize, Account<E>> {
        &self.accounts
    }

    fn block_number(&self) -> u32 {
        self.block_number
    }

    fn root_hash(&'a self) -> &'a E::Fr {
        &self.root_hash
    }
}

pub struct Tx<E: JubjubEngine> {
    pub from:               E::Fr,
    pub to:                 E::Fr,
    pub amount:             E::Fr,
    pub fee:                E::Fr,
    pub nonce:              E::Fr,
    pub good_until_block:   E::Fr,
    pub signature:          TransactionSignature<E>,
}

pub struct Block<E: JubjubEngine> {
    block_number:   u32,
    transactions:   Vec<Tx<E>>,
    new_root_hash:  E::Fr,
}
