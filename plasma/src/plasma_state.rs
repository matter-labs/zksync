// Plasma sidechain state implementation

use ff::{Field, PrimeField};
use rand::{Rand, thread_rng};
use pairing::{Engine};

use sapling_crypto::jubjub::JubjubEngine;
use sapling_crypto::circuit::ecc::EdwardsPoint;

pub trait PlasmaParams<E: JubjubEngine> {

    type Engine:    JubjubEngine;
    type Fs:        PrimeField;

    fn tree_height(&self) -> usize;
}

pub struct PlasmaBN256 {}

pub fn test() {

}



//struct Account<E: JubjubEngine> {
//    balance:    E::Fs,
//    nonce:      E::Fs,
//    pubkey:     EdwardsPoint<E>,
//}
//
//struct PlasmaState<E: JubjubEngine, TreeHeight: usize> {
//    // accounts
//    // Display
//    // apply(tx: Tx)
//}
//
//type AccountId = u32;
//
//type Value = u32;
//
//struct Tx {
//    from:   AccountId,
//    to:     AccountId,
//    value:  Value,
//    fee:    Value,
//}
//
//type ValuePacked = u32;
//
//// 112 bit
//struct TxPacked {
//    from:   AccountId,
//    to:     AccountId,
//    value:  ValuePacked,
//    fee:    ValuePacked,
//}
//
//type TxPubInput = TxPacked;
//
//#[derive(Clone)]
//struct Leaf<E: JubjubEngine, TreeHeight: usize> {
//
//    // state: 4 field elements
//    balance:    E::Fs,
//    nonce:      E::Fs,
//    pubkey:     EdwardsPoint<E>,
//
//    // Merkle auth path
//    merkle_path: [E::Fs; TreeHeight],
//}
//
//struct TxWitness<E: JubjubEngine> {
//    pub_input:              TxPubInput,
//    from_leaf:              Leaf<E>,
//    to_leaf:                Leaf<E>,
//    sig:                    EdwardsPoint<E>,
//    merkle_root_updated:    E::Fs,
//}
//
////use sapling_crypto::circuit::sha256::{sha256};
////use sapling_crypto::circuit::num::{AllocatedNum};
////use sapling_crypto::circuit::multipack::{pack_into_inputs};
////use num_bigint::BigUint;
//
//struct PlasmaUpdateCircuit<E: Engine> {
//    final_hash: Option<E::Fr>,
//}
//
////    let params = JubJubEngine::new();
////    let rng = &mut thread_rng();
////    let bits = (0..510).map(|_| bool::rand(rng)).collect::<Vec<_>>();
////    let personalization = Personalization::MerkleTree(31);
////    pedersen_hash::<Bn256, _>(personalization, bits.clone(), &params);
//
//impl<E: Engine> Circuit<E> for PlasmaUpdateCircuit<E> {
//    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
//
//        Ok(())
//    }
//}
