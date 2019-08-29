use crate::account::AccountContent;
use crate::account::AccountWitness;
use crate::allocated_structures::*;
use crate::element::{CircuitElement, CircuitPubkey};
use crate::operation::Operation;
use crate::utils::{allocate_audit_path, allocate_sum, pack_bits_to_element};
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::{Field, PrimeField};
use franklin_crypto::circuit::baby_eddsa::EddsaSignature;
use franklin_crypto::circuit::boolean::Boolean;
use franklin_crypto::circuit::ecc;
use franklin_crypto::circuit::sha256;

use bellman::groth16::generate_random_parameters;
use bellman::groth16::{
    create_random_proof, prepare_verifying_key, verify_proof, Parameters, Proof,
};
use franklin_crypto::circuit::expression::Expression;
use franklin_crypto::circuit::num::{AllocatedNum, Num};
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::polynomial_lookup::{do_the_lookup, generate_powers};
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};
use franklinmodels::params as franklin_constants;

#[derive(Clone)]
pub struct SmallCircuit<E: JubjubEngine> {
    pub a: Option<E::Fr>,
    pub b: Option<E::Fr>,
    pub c: Option<E::Fr>,
}

impl<E: JubjubEngine> Circuit<E> for SmallCircuit<E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        let c = AllocatedNum::alloc(cs.namespace(|| "c"), || self.c.grab())?;
        c.inputize(cs.namespace(|| "inputize c"))?;
        let b = AllocatedNum::alloc(cs.namespace(|| "b"), || self.b.grab())?;
        let a = AllocatedNum::alloc(cs.namespace(|| "a"), || self.a.grab())?;
        cs.enforce(
            || "a+b=c",
            |lc| lc + a.get_variable() + b.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + c.get_variable(),
        );

        Ok(())
    }
}

#[test]
#[ignore]
fn test_small_circuit_franklin() {
    use super::utils::public_data_commitment;

    use crate::circuit::FranklinCircuit;
    use crate::operation::*;
    use crate::utils::*;
    use bellman::Circuit;

    use ff::{BitIterator, Field, PrimeField};
    use franklin_crypto::alt_babyjubjub::AltJubjubBn256;

    use franklin_crypto::circuit::test::*;
    use franklin_crypto::eddsa::{PrivateKey, PublicKey};
    use franklin_crypto::jubjub::FixedGenerators;
    use franklinmodels::circuit::account::{
        Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
    };
    use franklinmodels::params as franklin_constants;

    use pairing::bn256::*;
    use rand::{Rng, SeedableRng, XorShiftRng};

    let mut rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);

    {
        let mut cs = TestConstraintSystem::<Bn256>::new();

        let instance = SmallCircuit {
            a: Some(Fr::from_str("5").unwrap()),
            b: Some(Fr::from_str("4").unwrap()),
            c: Some(Fr::from_str("9").unwrap()),
        };

        instance.synthesize(&mut cs).unwrap();

        println!("unconstrained {}", cs.find_unconstrained());

        println!("num constrained{}", cs.num_constraints());

        let err = cs.which_is_unsatisfied();
        if err.is_some() {
            panic!("ERROR satisfying in {}", err.unwrap());
        }
        let instance = SmallCircuit::<Bn256> {
            a: Some(Fr::from_str("5").unwrap()),
            b: Some(Fr::from_str("4").unwrap()),
            c: Some(Fr::from_str("9").unwrap()),
        };

        let tmp_cirtuit_params = generate_random_parameters(instance.clone(), &mut rng).unwrap();

        let proof = create_random_proof(instance, &tmp_cirtuit_params, &mut rng);
        if proof.is_err() {
            panic!("proof can not be created: {}", proof.err().unwrap());
            //             return Err(BabyProverErr::Other("proof.is_err()".to_owned()));
        }

        let p = proof.unwrap();

        let pvk = prepare_verifying_key(&tmp_cirtuit_params.vk);

        let success = verify_proof(&pvk, &p.clone(), &[Fr::from_str("9").unwrap()]);
        if success.is_err() {
            panic!(
                "Proof is verification failed with error {}",
                success.err().unwrap()
            );
            //             return Err(BabyProverErr::Other(
            //                 "Proof is verification failed".to_owned(),
            //             ));
        }
        if !success.unwrap() {
            panic!("Proof is invalid");
            //             return Err(BabyProverErr::Other("Proof is invalid".to_owned()));
        }
    }
}
