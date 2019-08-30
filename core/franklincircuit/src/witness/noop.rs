use super::utils::*;

use crate::operation::*;
use crate::utils::*;

use ff::{Field, PrimeField};

use crate::account::AccountWitness;

use franklinmodels::circuit::account::CircuitAccountTree;

use pairing::bn256::*;

pub fn noop_operation(
    tree: &CircuitAccountTree,
    acc_id: u32,
    sig_msg: &Fr,
    signature: Option<TransactionSignature<Bn256>>,
    signer_pub_key_x: &Fr,
    signer_pub_key_y: &Fr,
) -> Operation<Bn256> {
    let acc = tree.get(acc_id).unwrap();
    let account_address_fe = Fr::from_str(&acc_id.to_string()).unwrap();
    let token_fe = Fr::zero();
    let balance_value = match acc.subtree.get(0) {
        None => Fr::zero(),
        Some(bal) => bal.value.clone(),
    };
    let pubdata = vec![false; 64];
    let pubdata_chunks: Vec<_> = pubdata
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();
    let (audit_account, audit_balance) = get_audits(tree, acc_id, 0);

    Operation {
        new_root: Some(tree.root_hash()),
        tx_type: Some(Fr::from_str("0").unwrap()),
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(signer_pub_key_x.clone()),
        signer_pub_key_y: Some(signer_pub_key_y.clone()),

        args: OperationArguments {
            ethereum_key: Some(Fr::zero()),
            amount: Some(Fr::zero()),
            fee: Some(Fr::zero()),
            a: Some(Fr::zero()),
            b: Some(Fr::zero()),
            new_pub_key_hash: Some(Fr::zero()),
        },
        lhs: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(acc.nonce.clone()),
                    pub_key_hash: Some(acc.pub_key_hash.clone()),
                },
                account_path: audit_account.clone(),
                balance_value: Some(balance_value.clone()),
                balance_subtree_path: audit_balance.clone(),
            },
        },
        rhs: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(acc.nonce.clone()),
                    pub_key_hash: Some(acc.pub_key_hash.clone()),
                },
                account_path: audit_account.clone(),
                balance_value: Some(balance_value.clone()),
                balance_subtree_path: audit_balance.clone(),
            },
        },
    }
}
#[cfg(test)]
mod test {
    use super::*;
    use crate::witness::utils::public_data_commitment;

    use crate::circuit::FranklinCircuit;
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

    use rand::{Rng, SeedableRng, XorShiftRng};

    use franklinmodels::merkle_tree::PedersenHasher;

    #[test]
    fn test_noop_franklin() {
        let params = &AltJubjubBn256::new();
        let p_g = FixedGenerators::SpendingKeyGenerator;
        let validator_address_number = 7;
        let validator_address = Fr::from_str(&validator_address_number.to_string()).unwrap();
        let block_number = Fr::from_str("1").unwrap();
        let rng = &mut XorShiftRng::from_seed([0x3dbe_6258, 0x8d31_3d76, 0x3237_db17, 0xe5bc_0654]);
        let phasher = PedersenHasher::<Bn256>::default();

        let mut tree: CircuitAccountTree =
            CircuitAccountTree::new(franklin_constants::ACCOUNT_TREE_DEPTH as u32);

        let sender_sk = PrivateKey::<Bn256>(rng.gen());
        let sender_pk = PublicKey::from_private(&sender_sk, p_g, params);
        let sender_pub_key_hash = pub_key_hash(&sender_pk, &phasher);
        let (sender_x, sender_y) = sender_pk.0.into_xy();
        println!("x = {}, y = {}", sender_x, sender_y);

        // give some funds to sender and make zero balance for recipient
        let validator_sk = PrivateKey::<Bn256>(rng.gen());
        let validator_pk = PublicKey::from_private(&validator_sk, p_g, params);
        let validator_pub_key_hash = pub_key_hash(&validator_pk, &phasher);
        let (validator_x, validator_y) = validator_pk.0.into_xy();
        println!("x = {}, y = {}", validator_x, validator_y);
        let validator_leaf = CircuitAccount::<Bn256> {
            subtree: CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32),
            nonce: Fr::zero(),
            pub_key_hash: validator_pub_key_hash,
        };

        let mut validator_balances = vec![];
        for _ in 0..1 << *franklin_constants::BALANCE_TREE_DEPTH {
            validator_balances.push(Some(Fr::zero()));
        }
        tree.insert(validator_address_number, validator_leaf);

        let mut account_address: u32 = rng.gen();
        account_address %= tree.capacity();
        let token: u32 = 2;

        let sender_balance_before: u128 = 2000;

        let sender_balance_before_as_field_element =
            Fr::from_str(&sender_balance_before.to_string()).unwrap();

        let mut sender_balance_tree =
            CircuitBalanceTree::new(*franklin_constants::BALANCE_TREE_DEPTH as u32);
        sender_balance_tree.insert(
            token,
            Balance {
                value: sender_balance_before_as_field_element,
            },
        );

        let sender_leaf_initial = CircuitAccount::<Bn256> {
            subtree: sender_balance_tree,
            nonce: Fr::zero(),
            pub_key_hash: sender_pub_key_hash.clone(),
        };

        tree.insert(account_address, sender_leaf_initial);

        let sig_msg = Fr::from_str("2").unwrap(); //dummy sig msg cause skipped on partial_exit proof
        let mut sig_bits: Vec<bool> = BitIterator::new(sig_msg.into_repr()).collect();
        sig_bits.reverse();
        sig_bits.truncate(80);

        // println!(" capacity {}",<Bn256 as JubjubEngine>::Fs::Capacity);
        let signature = sign(&sig_bits, &sender_sk, p_g, params, rng);
        //assert!(tree.verify_proof(sender_leaf_number, sender_leaf.clone(), tree.merkle_path(sender_leaf_number)));

        let operation = noop_operation(
            &tree,
            validator_address_number,
            &sig_msg,
            signature,
            &sender_x,
            &sender_y,
        );
        let (_, validator_account_witness) = apply_fee(&mut tree, validator_address_number, 0, 0);
        let (validator_audit_path, _) = get_audits(&mut tree, validator_address_number, 0);

        let public_data_commitment = public_data_commitment::<Bn256>(
            &vec![false; 64],
            Some(tree.root_hash()),
            Some(tree.root_hash()),
            Some(validator_address),
            Some(block_number),
        );
        {
            let mut cs = TestConstraintSystem::<Bn256>::new();

            let instance = FranklinCircuit {
                operation_batch_size: 1,
                params,
                old_root: Some(tree.root_hash()),
                new_root: Some(tree.root_hash()),
                operations: vec![operation],
                pub_data_commitment: Some(public_data_commitment),
                block_number: Some(block_number),
                validator_account: validator_account_witness,
                validator_address: Some(validator_address),
                validator_balances: validator_balances,
                validator_audit_path: validator_audit_path,
            };

            instance.synthesize(&mut cs).unwrap();

            println!("{}", cs.find_unconstrained());

            println!("{}", cs.num_constraints());

            let err = cs.which_is_unsatisfied();
            if err.is_some() {
                panic!("ERROR satisfying in {}", err.unwrap());
            }
        }
    }

}
