#![allow(
    clippy::map_flatten,
    clippy::unnecessary_to_owned,
    clippy::derive_partial_eq_without_eq
)]
pub mod account;
pub mod allocated_structures;
pub mod circuit;
pub mod element;
pub mod exit_circuit;
pub mod operation;
#[cfg(test)]
#[cfg(feature = "playground")]
mod playground;
pub mod serialization;
pub mod signature;
pub mod utils;
pub mod witness;
