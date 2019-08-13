use bellman;

use pairing::bn256::*;
use rand::OsRng;
use franklin_crypto::alt_babyjubjub::AltJubjubBn256;
use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};
use bellman::groth16::generate_random_parameters;
use franklin_crypto::circuit::test::*;
use crate::circuit::FranklinCircuit;
use crate::operation::*;
use crate::account::AccountWitness;
use franklinmodels::params as franklin_constants;
use pairing::bn256::*;
use rand::{Rng, SeedableRng, XorShiftRng};
use bellman::Circuit;

#[test]
pub fn test_franklin_key() {
    // let p_g = FixedGenerators::SpendingKeyGenerator;
    let params = &AltJubjubBn256::new();
    // let rng = &mut XorShiftRng::from_seed([0x3dbe6258, 0x8d313d76, 0x3237db17, 0xe5bc0654]);
    let rng = &mut OsRng::new().unwrap();

    let empty_operation = Operation {
        new_root: None,
        tx_type: None,
        chunk: None,
        pubdata_chunk: None,
        signer_pub_key_x: None,
        signer_pub_key_y: None,
        sig_msg: None,
        signature: None,
        args: OperationArguments {
            a: None,
            b: None,
            amount: None,
            fee: None,
            new_pub_key_hash: None,
            ethereum_key: None
        },
        lhs: OperationBranch {
            address: None,
            token: None,
            witness: OperationBranchWitness {
                account_witness: AccountWitness { nonce: None, pub_key_hash: None },
                account_path: vec![None; franklin_constants::ACCOUNT_TREE_DEPTH],
                balance_value: None,
                balance_subtree_path: vec![None; *franklin_constants::BALANCE_TREE_DEPTH],
            }
        },
        rhs: OperationBranch {
            address: None,
            token: None,
            witness: OperationBranchWitness {
                account_witness: AccountWitness { nonce: None, pub_key_hash: None },
                account_path: vec![None; franklin_constants::ACCOUNT_TREE_DEPTH],
                balance_value: None,
                balance_subtree_path: vec![None; *franklin_constants::BALANCE_TREE_DEPTH],
            }
        }
    };


    let instance_for_generation: FranklinCircuit<'_, Bn256> = FranklinCircuit {
        params,
        operation_batch_size: franklin_constants::BLOCK_SIZE_CHUNKS,
        old_root: None,
        new_root: None,
        validator_address: None,
        block_number: None,
        pub_data_commitment: None,
        validator_balances: vec![None; 1 << (*franklin_constants::BALANCE_TREE_DEPTH as i32)],
        validator_audit_path: vec![None; franklin_constants::ACCOUNT_TREE_DEPTH],
        operations: vec![empty_operation; franklin_constants::BLOCK_SIZE_CHUNKS],
        validator_account: AccountWitness { nonce: None, pub_key_hash: None },
    };

    let mut cs = TestConstraintSystem::<Bn256>::new();

    instance_for_generation.synthesize(&mut cs).unwrap();
    assert_eq!(cs.num_constraints(), 1);
    println!("{}", cs.find_unconstrained());

    println!("number of constraints {}", cs.num_constraints());
    let err = cs.which_is_unsatisfied();
    if err.is_some() {
        panic!("ERROR satisfying in {}", err.unwrap());
    }

}

