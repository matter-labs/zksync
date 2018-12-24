#![allow(unused_imports)]
#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_must_use)]
#![allow(unused_assignments)]
#![allow(proc_macro_derive_resolution_fallback)]

pub mod runner;

mod rest_api;
mod state_keeper;
mod prover;
mod committer;
mod config;
mod mem_pool;
mod eth_watch;
mod storage;
mod schema;

extern crate plasma;

extern crate bellman;
extern crate pairing;
extern crate rand;
extern crate hex;
extern crate ff;
extern crate sapling_crypto;
extern crate crypto;
extern crate fnv;

extern crate futures;
extern crate web3;

extern crate hyper;
extern crate serde;
extern crate serde_json;

extern crate actix;
extern crate actix_web;

#[macro_use]
extern crate serde_derive;
extern crate serde_bytes;

#[macro_use]
extern crate diesel;
extern crate dotenv;
extern crate bigdecimal;
