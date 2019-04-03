//#![allow(unused_imports)]
//#![allow(unused_variables)]

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
extern crate reqwest;

#[macro_use]
extern crate serde_derive;
extern crate bigdecimal;
// #[macro_use]
// extern crate smart_default;

#[macro_use]
extern crate lazy_static;

pub mod primitives;
pub mod merkle_tree;
pub mod circuit;
pub mod vk_contract_generator;
pub mod models;
