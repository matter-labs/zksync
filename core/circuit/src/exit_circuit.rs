use crate::allocated_structures::*;
use crate::element::CircuitElement;
use crate::operation::{OperationBranch, OperationBranchWitness};
use crate::utils::pack_bits_to_element;
use bellman::{Circuit, ConstraintSystem, SynthesisError};
use ff::PrimeFieldRepr;
use ff::{Field, PrimeField};
use franklin_crypto::circuit::boolean::Boolean;
use franklin_crypto::circuit::sha256;

use crate::witness::utils::{apply_leaf_operation, get_audits};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use franklin_crypto::circuit::expression::Expression;
use franklin_crypto::circuit::num::AllocatedNum;
use franklin_crypto::circuit::pedersen_hash;
use franklin_crypto::circuit::Assignment;
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::utils::{append_be_fixed_width, be_bit_vector_into_bytes};
use models::circuit::CircuitAccountTree;
use models::node::{AccountId, Engine, Fr, TokenId};
use models::params::{
    ADDRESS_WIDTH, BALANCE_BIT_WIDTH, SUBTREE_HASH_WIDTH_PADDED, TOKEN_BIT_WIDTH,
};

#[derive(Clone)]
pub struct ZksyncExitCircuit<'a, E: JubjubEngine> {
    pub params: &'a E::Params,
    /// The old root of the tree
    pub pub_data_commitment: Option<E::Fr>,
    pub root_hash: Option<E::Fr>,
    pub account_audit_data: OperationBranch<E>,
}

// Implementation of our circuit:
impl<'a, E: JubjubEngine> Circuit<E> for ZksyncExitCircuit<'a, E> {
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
        let (state_root, _, _) =
            self.check_account_data(cs.namespace(|| "calculate account root"), &branch)?;

        // ensure root hash of state before applying operation is correct
        cs.enforce(
            || "account audit data corresponds to the root hash",
            |lc| lc + state_root.get_variable(),
            |lc| lc + CS::one(),
            |lc| lc + root_hash.get_variable(),
        );
        {
            // Now it's time to pack the initial SHA256 hash due to Ethereum BE encoding
            // and start rolling the hash

            let mut initial_hash_data: Vec<Boolean> = vec![];
            let root_hash_ce =
                CircuitElement::from_number_padded(cs.namespace(|| "root_hash_ce"), root_hash)?;
            initial_hash_data.extend(root_hash_ce.get_bits_be());
            initial_hash_data.extend(branch.account.address.get_bits_be());
            initial_hash_data.extend(branch.token.get_bits_be());
            initial_hash_data.extend(branch.balance.get_bits_be());

            let mut hash_block =
                sha256::sha256(cs.namespace(|| "sha256 of pub data"), &initial_hash_data)?;

            hash_block.reverse();
            hash_block.truncate(E::Fr::CAPACITY as usize);

            let final_hash = pack_bits_to_element(cs.namespace(|| "final_hash"), &hash_block)?;

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

impl<'a, E: JubjubEngine> ZksyncExitCircuit<'a, E> {
    fn check_account_data<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        cur: &AllocatedOperationBranch<E>,
    ) -> Result<(AllocatedNum<E>, Boolean, CircuitElement<E>), SynthesisError> {
        //first we prove calculate root of the subtree to obtain account_leaf_data:
        let (cur_account_leaf_bits, is_account_empty, subtree_root) = self
            .allocate_account_leaf_bits(
                cs.namespace(|| "allocate current_account_leaf_hash"),
                cur,
            )?;
        Ok((
            allocate_merkle_root(
                cs.namespace(|| "account_merkle_root"),
                &cur_account_leaf_bits,
                &cur.account_address.get_bits_le(),
                &cur.account_audit_path,
                self.params,
            )?,
            is_account_empty,
            subtree_root,
        ))
    }

    fn allocate_account_leaf_bits<CS: ConstraintSystem<E>>(
        &self,
        mut cs: CS,
        branch: &AllocatedOperationBranch<E>,
    ) -> Result<(Vec<Boolean>, Boolean, CircuitElement<E>), SynthesisError> {
        //first we prove calculate root of the subtree to obtain account_leaf_data:

        let balance_data = &branch.balance.get_bits_le();
        let balance_root = allocate_merkle_root(
            cs.namespace(|| "balance_subtree_root"),
            balance_data,
            &branch.token.get_bits_le(),
            &branch.balance_audit_path,
            self.params,
        )?;

        let subtree_root =
            CircuitElement::from_number_padded(cs.namespace(|| "subtree_root_ce"), balance_root)?;

        let mut account_data = vec![];
        account_data.extend(branch.account.nonce.get_bits_le());
        account_data.extend(branch.account.pub_key_hash.get_bits_le());
        account_data.extend(branch.account.address.get_bits_le());

        let account_data_packed =
            pack_bits_to_element(cs.namespace(|| "account_data_packed"), &account_data)?;

        let is_account_empty = Expression::equals(
            cs.namespace(|| "is_account_empty"),
            &account_data_packed,
            Expression::constant::<CS>(E::Fr::zero()),
        )?;
        account_data.extend(subtree_root.get_bits_le());
        Ok((account_data, Boolean::from(is_account_empty), subtree_root))
    }
}

fn allocate_merkle_root<E: JubjubEngine, CS: ConstraintSystem<E>>(
    mut cs: CS,
    leaf_bits: &[Boolean],
    index: &[Boolean],
    audit_path: &[AllocatedNum<E>],
    params: &E::Params,
) -> Result<AllocatedNum<E>, SynthesisError> {
    // only first bits of index are considered valuable
    assert!(index.len() >= audit_path.len());
    let index = &index[0..audit_path.len()];

    let account_leaf_hash = pedersen_hash::pedersen_hash(
        cs.namespace(|| "account leaf content hash"),
        pedersen_hash::Personalization::NoteCommitment,
        &leaf_bits,
        params,
    )?;
    // This is an injective encoding, as cur is a
    // point in the prime order subgroup.
    let mut cur_hash = account_leaf_hash.get_x().clone();

    // Ascend the merkle tree authentication path
    for (i, direction_bit) in index.iter().enumerate() {
        let cs = &mut cs.namespace(|| format!("from merkle tree hash {}", i));

        // "direction_bit" determines if the current subtree
        // is the "right" leaf at this depth of the tree.

        // Witness the authentication path element adjacent
        // at this depth.
        let path_element = &audit_path[i];

        // Swap the two if the current subtree is on the right
        let (xl, xr) = AllocatedNum::conditionally_reverse(
            cs.namespace(|| "conditional reversal of preimage"),
            &cur_hash,
            path_element,
            &direction_bit,
        )?;

        // We don't need to be strict, because the function is
        // collision-resistant. If the prover witnesses a congruency,
        // they will be unable to find an authentication path in the
        // tree with high probability.
        let mut preimage = vec![];
        preimage.extend(xl.into_bits_le(cs.namespace(|| "xl into bits"))?);
        preimage.extend(xr.into_bits_le(cs.namespace(|| "xr into bits"))?);

        // Compute the new subtree value
        cur_hash = pedersen_hash::pedersen_hash(
            cs.namespace(|| "computation of pedersen hash"),
            pedersen_hash::Personalization::MerkleTree(i),
            &preimage,
            params,
        )?
        .get_x()
        .clone(); // Injective encoding
    }

    Ok(cur_hash.clone())
}

pub fn create_exit_circuit(
    account_tree: &mut CircuitAccountTree,
    account_id: AccountId,
    token_id: TokenId,
) -> ZksyncExitCircuit<'static, Engine> {
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

    ZksyncExitCircuit {
        params: &models::params::JUBJUB_PARAMS,
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
    use bigdecimal::BigDecimal;
    use franklin_crypto::circuit::test::TestConstraintSystem;
    use models::circuit::account::CircuitAccount;
    use models::circuit::CircuitAccountTree;
    use models::node::Account;

    #[test]
    fn test_zksync_exit_circuit_correct_proof() {
        let test_account_id = 0xde;
        let token_id = 0x1d;
        let mut test_account = Account::default_with_address(
            &"abababababababababababababababababababab".parse().unwrap(),
        );
        test_account.set_balance(token_id, BigDecimal::from(0xbeef));
        test_account.nonce = 0xbabe;

        let mut circuit_account_tree =
            CircuitAccountTree::new(models::params::account_tree_depth() as u32);
        circuit_account_tree.insert(test_account_id, CircuitAccount::from(test_account));

        let zksync_exit_circuit =
            create_exit_circuit(&mut circuit_account_tree, test_account_id, token_id);

        let mut cs = TestConstraintSystem::<Engine>::new();
        zksync_exit_circuit.synthesize(&mut cs).unwrap();

        println!("unconstrained: {}", cs.find_unconstrained());
        println!("number of constraints {}", cs.num_constraints());
        if let Some(err) = cs.which_is_unsatisfied() {
            panic!("ERROR satisfying in {}", err);
        }
    }
}
