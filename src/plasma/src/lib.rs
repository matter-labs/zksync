//#![allow(unused_imports)]
//#![allow(unused_variables)]

use sapling_crypto;

#[macro_use]
extern crate serde_derive;

// #[macro_use]
// extern crate smart_default;

#[macro_use]
extern crate lazy_static;

pub mod circuit;
pub mod merkle_tree;
pub mod models;
pub mod primitives;
pub mod vk_contract_generator;
