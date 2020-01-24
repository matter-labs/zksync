use super::utils::*;

use crate::operation::SignatureData;
use crate::operation::*;
use ff::{Field, PrimeField};
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{
    append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element,
};
use models::node::operations::ChangePubkeyPriorityOp;
use models::params as franklin_constants;
use pairing::bn256::*;

pub struct ChangePubkeyOnchainData {
    pub account_id: u32,
    pub address: Fr,
    pub new_pubkeyhash: Fr,
}

#[derive(Debug, Clone)]
pub struct ChangePubkeyOnchainWitness<E: JubjubEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl<E: JubjubEngine> ChangePubkeyOnchainWitness<E> {
    pub fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.tx_type.unwrap(),
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.address.unwrap(),
            franklin_constants::ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.a.unwrap(),
            franklin_constants::SUCCESS_FLAG_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.new_pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.ethereum_key.unwrap(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        //        assert_eq!(pubdata_bits.len(), 37 * 8);
        pubdata_bits.resize(6 * franklin_constants::CHUNK_BIT_WIDTH, false);
        pubdata_bits
    }
}

pub fn apply_change_pubkey_onchain_tx(
    tree: &mut CircuitAccountTree,
    deposit: &ChangePubkeyPriorityOp,
) -> ChangePubkeyOnchainWitness<Bn256> {
    let change_pubkey_data = ChangePubkeyOnchainData {
        account_id: deposit.account_id.unwrap_or_default(),
        new_pubkeyhash: deposit.priority_op.new_pubkey_hash.to_fr(),
        address: eth_address_to_fr(&deposit.priority_op.eth_address),
    };

    apply_change_pubkey_onchain(tree, &change_pubkey_data, deposit.account_id.is_some())
}
pub fn apply_change_pubkey_onchain(
    tree: &mut CircuitAccountTree,
    pubkey_change: &ChangePubkeyOnchainData,
    success: bool,
) -> ChangePubkeyOnchainWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    println!("change pk onchain Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, pubkey_change.account_id, 0);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_id_fe = Fr::from_str(&pubkey_change.account_id.to_string()).unwrap();
    //calculate a and b
    let a = if success {
        Fr::from_str("1").unwrap()
    } else {
        Fr::zero()
    };
    let b = Fr::zero();

    //applying deposit
    let (account_witness_before, account_witness_after, balance_before, balance_after) =
        apply_leaf_operation(
            tree,
            pubkey_change.account_id,
            0,
            |acc| {
                if success {
                    assert_eq!(
                        acc.address, pubkey_change.address,
                        "successful operation account address mismatch"
                    );
                    acc.pub_key_hash = pubkey_change.new_pubkeyhash;
                }
            },
            |_| {},
        );

    let after_root = tree.root_hash();
    println!("change pk onchain After root = {}", after_root);
    let (audit_path_after, audit_balance_path_after) =
        get_audits(tree, pubkey_change.account_id, 0);

    ChangePubkeyOnchainWitness {
        before: OperationBranch {
            address: Some(account_id_fe),
            token: Some(Fr::zero()),
            witness: OperationBranchWitness {
                account_witness: account_witness_before,
                account_path: audit_path_before,
                balance_value: Some(balance_before),
                balance_subtree_path: audit_balance_path_before,
            },
        },
        after: OperationBranch {
            address: Some(account_id_fe),
            token: Some(Fr::zero()),
            witness: OperationBranchWitness {
                account_witness: account_witness_after,
                account_path: audit_path_after,
                balance_value: Some(balance_after),
                balance_subtree_path: audit_balance_path_after,
            },
        },
        args: OperationArguments {
            ethereum_key: Some(pubkey_change.address),
            amount_packed: Some(Fr::zero()),
            full_amount: Some(Fr::zero()),
            fee: Some(Fr::zero()),
            a: Some(a),
            b: Some(b),
            pub_nonce: Some(Fr::zero()),
            new_pub_key_hash: Some(pubkey_change.new_pubkeyhash),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("8").unwrap()),
    }
}

pub fn calculate_change_pubkey_operations_from_witness(
    deposit_witness: &ChangePubkeyOnchainWitness<Bn256>,
) -> Vec<Operation<Bn256>> {
    deposit_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .enumerate()
        .map(|(chunk_n, pubdata_chunk)| Operation {
            new_root: deposit_witness.after_root,
            tx_type: deposit_witness.tx_type,
            chunk: Some(Fr::from_str(&chunk_n.to_string()).unwrap()),
            pubdata_chunk: Some(pubdata_chunk),
            first_sig_msg: Some(Fr::zero()),
            second_sig_msg: Some(Fr::zero()),
            third_sig_msg: Some(Fr::zero()),
            signature_data: SignatureData {
                r_packed: vec![Some(false); 256],
                s: vec![Some(false); 256],
            },
            signer_pub_key_packed: vec![Some(false); 256],
            args: deposit_witness.args.clone(),
            lhs: deposit_witness.before.clone(),
            rhs: deposit_witness.after.clone(),
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::witness::test_utils::{check_circuit, test_genesis_plasma_state};
    use bigdecimal::BigDecimal;
    use ff::Field;
    use models::node::priority_ops::ChangePubKeyPriority;
    use models::node::{Account, Address, Deposit, PubKeyHash};
    use models::primitives::pack_bits_into_bytes_in_order;

    #[test]
    #[ignore]
    fn test_change_pubkey_onchain_success() {
        let change_pkhash_to_account_id = 0xc1;
        let change_pkhash_to_account_address =
            "9090909090909090909090909090909090909090".parse().unwrap();
        let (mut plasma_state, mut witness_accum) = test_genesis_plasma_state(vec![(
            change_pkhash_to_account_id,
            Account::default_with_address(&change_pkhash_to_account_address),
        )]);

        let change_pkhash_op = ChangePubkeyPriorityOp {
            priority_op: ChangePubKeyPriority {
                new_pubkey_hash: PubKeyHash::from_hex(
                    "sync:0808080808080808080808080808080808080808",
                )
                .unwrap(),
                eth_address: change_pkhash_to_account_address,
            },
            account_id: Some(change_pkhash_to_account_id),
        };

        println!("node root hash before op: {:?}", plasma_state.root_hash());
        plasma_state.apply_change_pubkey_priority_op(&change_pkhash_op);
        println!("node root hash after op: {:?}", plasma_state.root_hash());
        println!(
            "node pubdata: {}",
            hex::encode(&change_pkhash_op.get_public_data())
        );

        let change_pkhash_witness =
            apply_change_pubkey_onchain_tx(&mut witness_accum.account_tree, &change_pkhash_op);
        let change_pkhash_operations =
            calculate_change_pubkey_operations_from_witness(&change_pkhash_witness);
        let pub_data_from_witness = change_pkhash_witness.get_pubdata();

        println!("Change pk onchain witness: {:#?}", change_pkhash_witness);

        println!(
            "pubdata from witness: {}",
            hex::encode(&pack_bits_into_bytes_in_order(
                pub_data_from_witness.clone()
            ))
        );

        witness_accum.add_operation_with_pubdata(change_pkhash_operations, pub_data_from_witness);
        witness_accum.collect_fees(&Vec::new());
        witness_accum.calculate_pubdata_commitment();

        assert_eq!(
            plasma_state.root_hash(),
            witness_accum
                .root_after_fees
                .expect("witness accum after root hash empty"),
            "root hash in state keeper and witness generation code mismatch"
        );

        check_circuit(witness_accum.into_circuit_instance());
    }
}
