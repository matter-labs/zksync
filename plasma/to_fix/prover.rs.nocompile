use std::error::Error;
use std::fmt::Debug;
use super::state::{Account, State, Block};

use sapling_crypto::alt_babyjubjub::{JubjubEngine};

pub trait Prover<E: JubjubEngine>: Sized {

    type Err: Error + Sized;
    type Proof: Debug + Sized;
    type EncodedProof: Debug + Clone;

    // fn new<'a>(initial_state: &State<'a, E>) -> Result<Self, Self::Err>;

    fn encode_proof(proof: &Self::Proof) -> Result<Self::EncodedProof, Self::Err>;
    fn encode_transactions(block: &Block<E>) -> Result<Vec<u8>, Self::Err>;

    fn apply_and_prove(&mut self, block: &Block<E>) -> Result<Self::Proof, Self::Err>;
    
    // will be used laters with multiple parallel provers
    fn apply(&mut self, block: &Block<E>) -> Result<Self::Proof, Self::Err> {
        unimplemented!()
    }
}
