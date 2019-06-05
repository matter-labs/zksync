//#![allow(unused_imports)]
//#![allow(unused_variables)]

extern crate bellman;
extern crate crypto;
extern crate ff;
extern crate fnv;
extern crate hex;
extern crate pairing;
extern crate rand;
extern crate sapling_crypto;

extern crate crossbeam;
extern crate crossbeam_utils;
extern crate futures;
extern crate futures_cpupool;
extern crate rayon;
extern crate tokio;

extern crate ethabi;
extern crate ethereum_tx_sign;
extern crate ethereum_types;
extern crate rustc_hex;
extern crate web3;

extern crate hyper;
extern crate reqwest;
extern crate serde;

#[macro_use]
extern crate serde_derive;
extern crate bigdecimal;
// #[macro_use]
// extern crate smart_default;

#[macro_use]
extern crate lazy_static;

pub mod circuit;
pub mod merkle_tree;
pub mod models;
pub mod primitives;
pub mod vk_contract_generator;
