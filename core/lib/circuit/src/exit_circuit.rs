// External deps
use crypto::{digest::Digest, sha2::Sha256};
use zksync_crypto::franklin_crypto::{
    bellman::{
        pairing::ff::{Field, PrimeField, PrimeFieldRepr},
        Circuit, ConstraintSystem, SynthesisError,
    },
    circuit::{boolean::Boolean, num::AllocatedNum, sha256, Assignment},
    rescue::RescueEngine,
};
// Workspace deps
use zksync_crypto::{
    circuit::{
        utils::{append_be_fixed_width, be_bit_vector_into_bytes},
        CircuitAccountTree,
    },
    params::{
        ACCOUNT_ID_BIT_WIDTH, ADDRESS_WIDTH, BALANCE_BIT_WIDTH, FR_BIT_WIDTH_PADDED,
        SUBTREE_HASH_WIDTH_PADDED, TOKEN_BIT_WIDTH,
    },
    Engine, Fr,
};
use zksync_types::{AccountId, TokenId};
// Local deps
use crate::{
    allocated_structures::*,
    circuit::check_account_data,
    element::CircuitElement,
    operation::{OperationBranch, OperationBranchWitness},
    witness::utils::{apply_leaf_operation, get_audits},
};

#[derive(Clone)]
pub struct ZkSyncExitCircuit<'a, E: RescueEngine> {
    pub params: &'a E::Params,
    /// The old root of the tree
    pub pub_data_commitment: Option<E::Fr>,
    pub root_hash: Option<E::Fr>,
    pub account_audit_data: OperationBranch<E>,
}

// Implementation of our circuit:
impl<'a, E: RescueEngine> Circuit<E> for ZkSyncExitCircuit<'a, E> {
    fn synthesize<CS: ConstraintSystem<E>>(self, cs: &mut CS) -> Result<(), SynthesisError> {
        // this is only public input to our circuit
        let public_data_commitment =
            AllocatedNum::alloc(cs.namespace(|| "public_data_commitment"), || {
                self.pub_data_commitment.grab()
            })?;
        public_data_commitment.inputize(cs.namespace(|| "inputize pub_data"))?;

        let root_hash =
            AllocatedNum::alloc(cs.namespace(|| "root_hash"), || self.root_hash.grab())?;

        let branch = AllocatedOperationBranch::from_witness(
            cs.namespace(|| "lhs"),
            &self.account_audit_data,
        )?;
        // calculate root for given account data
        let (state_root, _, _) = check_account_data(
            cs.namespace(|| "calculate account root"),
            &branch,
            zksync_crypto::params::account_tree_depth(),
            self.params,
        )?;

        // ensure root hash of state is correct
        cs.enforce(
            || "account audit data corresponds to the root hash",
            |lc| lc + state_root.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + root_hash.get_variable(),
        );
        {
            let mut initial_hash_data: Vec<Boolean> = vec![];
            let root_hash_ce =
                CircuitElement::from_number(cs.namespace(|| "root_hash_ce"), root_hash)?;
            initial_hash_data.extend(root_hash_ce.into_padded_be_bits(FR_BIT_WIDTH_PADDED));
            initial_hash_data.extend(branch.account_id.get_bits_be());
            initial_hash_data.extend(branch.account.address.get_bits_be());
            initial_hash_data.extend(branch.token.get_bits_be());
            initial_hash_data.extend(branch.balance.get_bits_be());

            let mut hash_block =
                sha256::sha256(cs.namespace(|| "sha256 of pub data"), &initial_hash_data)?;

            hash_block.reverse();
            hash_block.truncate(E::Fr::CAPACITY as usize);

            let final_hash =
                AllocatedNum::pack_bits_to_element(cs.namespace(|| "final_hash"), &hash_block)?;

            cs.enforce(
                || "enforce external data hash equality",
                |lc| lc + public_data_commitment.get_variable(),
                |lc| lc + CS::one(),
                |lc| lc + final_hash.get_variable(),
            );
        }
        Ok(())
    }
}

pub fn create_exit_circuit_with_public_input(
    account_tree: &mut CircuitAccountTree,
    account_id: AccountId,
    token_id: TokenId,
) -> ZkSyncExitCircuit<'static, Engine> {
    let account_address_fe = Fr::from_str(&account_id.to_string()).unwrap();
    let token_id_fe = Fr::from_str(&token_id.to_string()).unwrap();
    let root_hash = account_tree.root_hash();
    let (account_witness, _, balance, _) = apply_leaf_operation(
        account_tree,
        account_id,
        u32::from(token_id),
        |_| {},
        |_| {},
    );
    let (audit_path, audit_balance_path) =
        get_audits(account_tree, account_id, u32::from(token_id));

    let mut pubdata_commitment = Vec::new();
    append_be_fixed_width(
        &mut pubdata_commitment,
        &root_hash,
        SUBTREE_HASH_WIDTH_PADDED,
    );
    append_be_fixed_width(
        &mut pubdata_commitment,
        &account_address_fe,
        ACCOUNT_ID_BIT_WIDTH,
    );
    let account_address = account_tree
        .get(account_id)
        .expect("account should be in the tree")
        .address;
    append_be_fixed_width(&mut pubdata_commitment, &account_address, ADDRESS_WIDTH);
    append_be_fixed_width(&mut pubdata_commitment, &token_id_fe, TOKEN_BIT_WIDTH);
    append_be_fixed_width(&mut pubdata_commitment, &balance, BALANCE_BIT_WIDTH);

    let mut h = Sha256::new();

    let bytes_to_hash = be_bit_vector_into_bytes(&pubdata_commitment);
    h.input(&bytes_to_hash);
    let mut hash_result = [0u8; 32];
    h.result(&mut hash_result[..]);
    hash_result[0] &= 0x1f; // temporary solution, this nullifies top bits to be encoded into field element correctly

    let mut repr = Fr::zero().into_repr();
    repr.read_be(&hash_result[..])
        .expect("pack hash as field element");

    let pub_data_commitment = Fr::from_repr(repr).unwrap();

    ZkSyncExitCircuit {
        params: &zksync_crypto::params::RESCUE_PARAMS,
        pub_data_commitment: Some(pub_data_commitment),
        root_hash: Some(root_hash),
        account_audit_data: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_id_fe),
            witness: OperationBranchWitness {
                account_witness,
                account_path: audit_path,
                balance_value: Some(balance),
                balance_subtree_path: audit_balance_path,
            },
        },
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use num::BigUint;
    use zksync_crypto::circuit::account::CircuitAccount;
    use zksync_crypto::circuit::CircuitAccountTree;
    use zksync_crypto::franklin_crypto::circuit::test::TestConstraintSystem;
    use zksync_types::Account;

    #[test]
    #[ignore]
    fn test_zksync_exit_circuit_correct_proof() {
        let test_account_id = 0xde;
        let token_id = 0x1d;
        let mut test_account = Account::default_with_address(
            &"abababababababababababababababababababab".parse().unwrap(),
        );
        test_account.set_balance(token_id, BigUint::from(0xbeefu32));
        test_account.nonce = 0xbabe;

        let mut circuit_account_tree =
            CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());
        circuit_account_tree.insert(test_account_id, CircuitAccount::from(test_account));

        let zksync_exit_circuit = create_exit_circuit_with_public_input(
            &mut circuit_account_tree,
            test_account_id,
            token_id,
        );

        let mut cs = TestConstraintSystem::<Engine>::new();
        zksync_exit_circuit.synthesize(&mut cs).unwrap();

        println!("unconstrained: {}", cs.find_unconstrained());
        println!("number of constraints {}", cs.num_constraints());
        if let Some(err) = cs.which_is_unsatisfied() {
            panic!("ERROR satisfying in {}", err);
        }
    }
}
