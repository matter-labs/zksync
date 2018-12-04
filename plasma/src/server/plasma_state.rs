use std::collections::{hash_map, HashMap};

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::super::balance_tree;
use super::super::circuit::baby_plasma::TransactionSignature;

pub type Account<E> = balance_tree::Leaf<E>;

pub trait State<'a, E: JubjubEngine> {  
    fn accounts_iter(&'a self) -> hash_map::Iter<'a, usize, Account<E>>;
    fn block_number(&self) -> u32;
    fn root_hash(&'a self) -> &'a E::Fr;
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
