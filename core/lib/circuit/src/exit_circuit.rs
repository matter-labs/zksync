// External deps
use crypto::{digest::Digest, sha2::Sha256};
use zksync_crypto::franklin_crypto::{
    bellman::{
        pairing::ff::{Field, PrimeField, PrimeFieldRepr},
        Circuit, ConstraintSystem, SynthesisError,
    },
    circuit::{boolean::Boolean, expression::Expression, num::AllocatedNum, sha256, Assignment},
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
        MIN_NFT_TOKEN_ID, NFT_STORAGE_ACCOUNT_ID, SERIAL_ID_WIDTH, SUBTREE_HASH_WIDTH_PADDED,
        TOKEN_BIT_WIDTH,
    },
    Engine, Fr,
};
use zksync_types::{AccountId, TokenId, H256};
// Local deps
use crate::witness::utils::fr_from;
use crate::{
    allocated_structures::*,
    circuit::{check_account_data, hash_nft_content_to_balance_type},
    element::CircuitElement,
    operation::{OperationBranch, OperationBranchWitness},
    utils::boolean_or,
    witness::utils::{apply_leaf_operation, get_audits},
};

#[derive(Clone)]
pub struct ZkSyncExitCircuit<'a, E: RescueEngine> {
    pub params: &'a E::Params,
    /// The old root of the tree
    pub pub_data_commitment: Option<E::Fr>,
    pub root_hash: Option<E::Fr>,
    pub account_audit_data: OperationBranch<E>,

    pub special_account_audit_data: OperationBranch<E>,
    pub creator_account_audit_data: OperationBranch<E>,
    pub serial_id: Option<E::Fr>,
    pub content_hash: Vec<Option<E::Fr>>,
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
            cs.namespace(|| "branch"),
            &self.account_audit_data,
        )?;
        // calculate root for given account data
        let (state_root, _, _) = check_account_data(
            cs.namespace(|| "calculate account root"),
            &branch,
            zksync_crypto::params::account_tree_depth(),
            self.params,
        )?;

        let serial_id = CircuitElement::from_fe_with_known_length(
            cs.namespace(|| "serial_id"),
            || self.serial_id.grab(),
            SERIAL_ID_WIDTH,
        )?;
        let content_hash = self
            .content_hash
            .iter()
            .enumerate()
            .map(|(idx, content_hash_bit)| {
                CircuitElement::from_fe_with_known_length(
                    cs.namespace(|| format!("content_hash bit with index {}", idx)),
                    || content_hash_bit.grab(),
                    1,
                )
            })
            .collect::<Result<Vec<_>, SynthesisError>>()?;

        let special_account_branch = AllocatedOperationBranch::from_witness(
            cs.namespace(|| "special_account_branch"),
            &self.special_account_audit_data,
        )?;
        // calculate root for given account data
        let (state_root_special_branch, _, _) = check_account_data(
            cs.namespace(|| "calculate account root (special_account_branch)"),
            &special_account_branch,
            zksync_crypto::params::account_tree_depth(),
            self.params,
        )?;

        let creator_account_branch = AllocatedOperationBranch::from_witness(
            cs.namespace(|| "creator_account_branch"),
            &self.creator_account_audit_data,
        )?;
        // calculate root for given account data
        let (state_root_creator_branch, _, _) = check_account_data(
            cs.namespace(|| "calculate account root (creator_account_branch)"),
            &creator_account_branch,
            zksync_crypto::params::account_tree_depth(),
            self.params,
        )?;

        let allocated_roots = vec![
            state_root.clone(),
            state_root_special_branch,
            state_root_creator_branch,
        ];
        for i in 1..allocated_roots.len() {
            let allocated_roots_are_equal = Boolean::from(AllocatedNum::equals(
                cs.namespace(|| format!("allocated_roots {} and {} are equals", i - 1, i)),
                &allocated_roots[i - 1],
                &allocated_roots[i],
            )?);
            Boolean::enforce_equal(
                cs.namespace(|| format!("allocated_roots {} and {} are valid", i - 1, i)),
                &allocated_roots_are_equal,
                &Boolean::constant(true),
            )?;
        }

        let is_special_nft_storage_account = Boolean::from(Expression::equals(
            cs.namespace(|| "is_special_nft_storage_account"),
            &special_account_branch.account_id.get_number(),
            Expression::u64::<CS>(NFT_STORAGE_ACCOUNT_ID.0.into()),
        )?);
        Boolean::enforce_equal(
            cs.namespace(|| "is_special_nft_storage_account should be true"),
            &is_special_nft_storage_account,
            &Boolean::constant(true),
        )?;

        let is_token_valid = Boolean::from(Expression::equals(
            cs.namespace(|| "is_token_valid"),
            &branch.token.get_number(),
            &special_account_branch.token.get_number(),
        )?);
        Boolean::enforce_equal(
            cs.namespace(|| "is_token_valid should be true"),
            &is_token_valid,
            &Boolean::constant(true),
        )?;

        let nft_content_as_balance = hash_nft_content_to_balance_type(
            cs.namespace(|| "hash_nft_content_to_balance_type"),
            &creator_account_branch.account_id,
            &serial_id,
            &content_hash,
            self.params,
        )?;
        let is_nft_content_valid = Boolean::from(Expression::equals(
            cs.namespace(|| "is_nft_content_valid"),
            &nft_content_as_balance.get_number(),
            &special_account_branch.balance.get_number(),
        )?);
        let min_nft_token_id_number =
            AllocatedNum::alloc(cs.namespace(|| "min_nft_token_id number"), || {
                Ok(E::Fr::from_str(&MIN_NFT_TOKEN_ID.to_string()).unwrap())
            })?;
        min_nft_token_id_number.assert_number(
            cs.namespace(|| "assert min_nft_token_id is a constant"),
            &E::Fr::from_str(&MIN_NFT_TOKEN_ID.to_string()).unwrap(),
        )?;
        let min_nft_token_id = CircuitElement::from_number_with_known_length(
            cs.namespace(|| "min_nft_token_id circuit element"),
            min_nft_token_id_number,
            TOKEN_BIT_WIDTH,
        )?;
        let is_fungible_token = CircuitElement::less_than_fixed(
            cs.namespace(|| "is_fungible_token"),
            &branch.token,
            &min_nft_token_id,
        )?;
        let nft_content_valid_or_fungible_token = boolean_or(
            cs.namespace(|| "nft_content_valid_or_fungible_token"),
            &is_nft_content_valid,
            &is_fungible_token,
        )?;
        Boolean::enforce_equal(
            cs.namespace(|| "nft_content_valid_or_fungible_token should be true"),
            &nft_content_valid_or_fungible_token,
            &Boolean::constant(true),
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
            initial_hash_data.extend(creator_account_branch.account_id.get_bits_be());
            initial_hash_data.extend(creator_account_branch.account.address.get_bits_be());
            initial_hash_data.extend(serial_id.get_bits_be());
            initial_hash_data.extend(content_hash.iter().map(|bit| bit.get_bits_be()).flatten());

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
    nft_creator_id: AccountId,
    nft_serial_id: u32,
    nft_content_hash: H256,
) -> ZkSyncExitCircuit<'static, Engine> {
    let account_address_fe = Fr::from_str(&account_id.to_string()).unwrap();
    let creator_account_address_fe = Fr::from_str(&nft_creator_id.to_string()).unwrap();
    let token_id_fe = Fr::from_str(&token_id.to_string()).unwrap();
    let serial_id_fe = Fr::from_str(&nft_serial_id.to_string()).unwrap();
    let root_hash = account_tree.root_hash();
    let (account_witness, _, balance, _) =
        apply_leaf_operation(account_tree, *account_id, *token_id as u32, |_| {}, |_| {});
    let (audit_path, audit_balance_path) = get_audits(account_tree, *account_id, *token_id as u32);

    let (special_account_witness, _, special_account_balance, _) = apply_leaf_operation(
        account_tree,
        NFT_STORAGE_ACCOUNT_ID.0,
        *token_id as u32,
        |_| {},
        |_| {},
    );
    let (special_account_audit_path, special_account_audit_balance_path) =
        get_audits(account_tree, NFT_STORAGE_ACCOUNT_ID.0, *token_id as u32);

    let (creator_account_witness, _, creator_account_balance, _) = apply_leaf_operation(
        account_tree,
        *nft_creator_id,
        *token_id as u32,
        |_| {},
        |_| {},
    );
    let (creator_account_audit_path, creator_account_audit_balance_path) =
        get_audits(account_tree, *nft_creator_id, *token_id as u32);

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
        .get(*account_id)
        .expect("account should be in the tree")
        .address;
    append_be_fixed_width(&mut pubdata_commitment, &account_address, ADDRESS_WIDTH);
    append_be_fixed_width(&mut pubdata_commitment, &token_id_fe, TOKEN_BIT_WIDTH);
    append_be_fixed_width(&mut pubdata_commitment, &balance, BALANCE_BIT_WIDTH);

    append_be_fixed_width(
        &mut pubdata_commitment,
        &creator_account_address_fe,
        ACCOUNT_ID_BIT_WIDTH,
    );
    let creator_address = account_tree
        .get(*nft_creator_id)
        .expect("nft creator id account should be in the tree")
        .address;
    append_be_fixed_width(&mut pubdata_commitment, &creator_address, ADDRESS_WIDTH);
    append_be_fixed_width(&mut pubdata_commitment, &serial_id_fe, SERIAL_ID_WIDTH);
    let content_hash_as_vec: Vec<Option<Fr>> = nft_content_hash
        .as_bytes()
        .iter()
        .map(|input_byte| {
            let mut byte_as_bits = vec![];
            let mut byte = *input_byte;
            for _ in 0..8 {
                byte_as_bits.push(byte & 1);
                byte /= 2;
            }
            byte_as_bits.reverse();
            byte_as_bits
        })
        .flatten()
        .map(|bit| Some(fr_from(&bit)))
        .collect();
    for bit in &content_hash_as_vec {
        append_be_fixed_width(&mut pubdata_commitment, &bit.unwrap(), 1);
    }

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
        special_account_audit_data: OperationBranch {
            address: Some(fr_from(&NFT_STORAGE_ACCOUNT_ID)),
            token: Some(token_id_fe),
            witness: OperationBranchWitness {
                account_witness: special_account_witness,
                account_path: special_account_audit_path,
                balance_value: Some(special_account_balance),
                balance_subtree_path: special_account_audit_balance_path,
            },
        },
        creator_account_audit_data: OperationBranch {
            address: Some(creator_account_address_fe),
            token: Some(token_id_fe),
            witness: OperationBranchWitness {
                account_witness: creator_account_witness,
                account_path: creator_account_audit_path,
                balance_value: Some(creator_account_balance),
                balance_subtree_path: creator_account_audit_balance_path,
            },
        },
        serial_id: Some(serial_id_fe),
        content_hash: content_hash_as_vec,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use num::BigUint;
    use zksync_crypto::circuit::account::CircuitAccount;
    use zksync_crypto::circuit::CircuitAccountTree;
    use zksync_crypto::convert::FeConvert;
    use zksync_crypto::franklin_crypto::bellman::pairing::bn256::{Bn256, Fr};
    use zksync_crypto::franklin_crypto::circuit::test::TestConstraintSystem;
    use zksync_crypto::params::{NFT_STORAGE_ACCOUNT_ADDRESS, NFT_STORAGE_ACCOUNT_ID};
    use zksync_crypto::rescue_poseidon::rescue_hash;
    use zksync_types::{Account, Nonce};

    #[test]
    #[ignore]
    fn test_zksync_exit_circuit_correct_proof() {
        let test_account_id = AccountId(0xde);
        let token_id = TokenId(0x1d);
        let mut test_account = Account::default_with_address(
            &"abababababababababababababababababababab".parse().unwrap(),
        );
        test_account.set_balance(token_id, BigUint::from(0xbeefu32));
        test_account.nonce = Nonce(0xbabe);

        let mut circuit_account_tree =
            CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());
        circuit_account_tree.insert(*test_account_id, CircuitAccount::from(test_account));

        let zksync_exit_circuit = create_exit_circuit_with_public_input(
            &mut circuit_account_tree,
            test_account_id,
            token_id,
            Default::default(),
            Default::default(),
            Default::default(),
        );

        let mut cs = TestConstraintSystem::<Engine>::new();
        zksync_exit_circuit.synthesize(&mut cs).unwrap();

        println!("unconstrained: {}", cs.find_unconstrained());
        println!("number of constraints {}", cs.num_constraints());
        if let Some(err) = cs.which_is_unsatisfied() {
            panic!("ERROR satisfying in {}", err);
        }
    }

    #[test]
    #[ignore]
    fn test_zksync_exit_circuit_nft_token_correct_proof() {
        let test_account_id = AccountId(0xde);
        let token_id = TokenId(MIN_NFT_TOKEN_ID);
        let mut test_account = Account::default_with_address(
            &"abababababababababababababababababababab".parse().unwrap(),
        );
        test_account.set_balance(token_id, BigUint::from(0xbeefu32));
        test_account.nonce = Nonce(0xbabe);

        let mut circuit_account_tree =
            CircuitAccountTree::new(zksync_crypto::params::account_tree_depth());
        circuit_account_tree.insert(*test_account_id, CircuitAccount::from(test_account));

        let serial_id = 123;
        let content_hash = H256::random();

        fn content_to_store_as_balance_as_bytes_be(
            creator_account_id: u32,
            serial_id: u32,
            content_hash: H256,
        ) -> Vec<u8> {
            let mut lhs_be_bits = vec![];
            lhs_be_bits.extend_from_slice(&creator_account_id.to_be_bytes());
            lhs_be_bits.extend_from_slice(&serial_id.to_be_bytes());
            lhs_be_bits.extend_from_slice(&content_hash.as_bytes()[..16]);
            let lhs_fr =
                Fr::from_hex(&format!("0x{}", hex::encode(&lhs_be_bits))).expect("lhs as Fr");

            let mut rhs_be_bits = vec![];
            rhs_be_bits.extend_from_slice(&content_hash.as_bytes()[16..]);
            let rhs_fr =
                Fr::from_hex(&format!("0x{}", hex::encode(&rhs_be_bits))).expect("rhs as Fr");

            let hash_result = rescue_hash::<Bn256, 2>(&[lhs_fr, rhs_fr]);

            let mut result_bytes = vec![0u8; 16];
            result_bytes.extend_from_slice(&hash_result[0].to_bytes()[16..]);

            result_bytes
        }
        let content_to_store_as_bytes_be =
            content_to_store_as_balance_as_bytes_be(*test_account_id, serial_id, content_hash);
        let mut special_account =
            Account::create_account(NFT_STORAGE_ACCOUNT_ID, *NFT_STORAGE_ACCOUNT_ADDRESS).0;
        special_account.set_balance(
            token_id,
            BigUint::from_bytes_be(&content_to_store_as_bytes_be.as_slice()[16..]),
        );
        circuit_account_tree.insert(
            NFT_STORAGE_ACCOUNT_ID.0,
            CircuitAccount::from(special_account),
        );

        let zksync_exit_circuit = create_exit_circuit_with_public_input(
            &mut circuit_account_tree,
            test_account_id,
            token_id,
            test_account_id,
            serial_id,
            content_hash,
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
