#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_must_use)]
#![allow(unused_assignments)]

extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate hex;
extern crate ff;
extern crate sapling_crypto;
extern crate crypto;
extern crate fnv;

extern crate futures;
extern crate futures_cpupool;
extern crate crossbeam;
extern crate crossbeam_utils;
extern crate rayon;
extern crate tokio;

extern crate ethereum_types;
extern crate ethabi;
extern crate rustc_hex;
extern crate web3;
extern crate ethereum_tx_sign;

extern crate hyper;
extern crate serde;
extern crate serde_json;
extern crate reqwest;

#[macro_use]
extern crate serde_derive;

pub mod primitives;
pub mod sparse_merkle_tree;
pub mod balance_tree;
pub mod circuit;
pub mod vk_contract_generator;
// pub mod server;