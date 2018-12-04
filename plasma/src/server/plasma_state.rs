use std::collections::HashMap;

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

use super::super::balance_tree;
use super::super::circuit::baby_plasma::TransactionSignature;

pub type Account<E> = balance_tree::Leaf<E>;

pub struct State<E: JubjubEngine> {
    
    // current state of accounts
    pub accounts:       HashMap<usize, Account<E>>,

    // current block number
    pub block_number:   u32,

    // current root hash
    pub root_hash:      E::Fr,
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
