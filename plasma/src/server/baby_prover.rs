use std::error::Error;
use super::plasma_state::{State, Block};
use super::prover::{Prover};

use pairing::bn256::Bn256;
use bellman::groth16::Proof;

pub struct BabyProver {

}

type BabyProof = Proof<Bn256>;

// TODO: replace with what is proper for BabyProver
type BabyProverErr = std::io::Error;

impl Prover<Bn256> for BabyProver {

    type Err = BabyProverErr;
    type Proof = BabyProof;
    
    fn new(initial_state: &State<Bn256>) -> Result<Self, Self::Err> {
        Ok(Self{})
    }

    fn encode_proof(block: &Self::Proof) -> Result<Vec<u8>, Self::Err> {

        // uint256[8] memory in_proof
        // see contracts/Verifier.sol:44

        // TODO: implement
        unimplemented!()        
    }

    fn encode_transactions(block: &Block<Bn256>) -> Result<Vec<u8>, Self::Err> {
        // TODO: implement
        unimplemented!()
    }

    fn apply_and_prove(&mut self, block: &Block<Bn256>) -> Result<Self::Proof, Self::Err> {
        // TODO: implement
        unimplemented!()
    }
    
}