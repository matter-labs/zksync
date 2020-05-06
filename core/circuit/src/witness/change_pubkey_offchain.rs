use super::utils::*;
use crate::franklin_crypto::bellman::pairing::bn256::*;
use crate::franklin_crypto::rescue::RescueEngine;
use crate::operation::SignatureData;
use crate::operation::*;
use crypto_exports::ff::{Field, PrimeField};
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{
    append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element,
};
use models::node::operations::ChangePubKeyOp;
use models::params as franklin_constants;

pub struct ChangePubkeyOffChainData {
    pub account_id: u32,
    pub address: Fr,
    pub new_pubkey_hash: Fr,
    pub nonce: Fr,
}

pub struct ChangePubkeyOffChainWitness<E: RescueEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl<E: RescueEngine> ChangePubkeyOffChainWitness<E> {
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
            &self.args.new_pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );

        assert!(pubdata_bits.len() <= ChangePubKeyOp::CHUNKS * franklin_constants::CHUNK_BIT_WIDTH);
        pubdata_bits.resize(
            ChangePubKeyOp::CHUNKS * franklin_constants::CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }
}

pub fn apply_change_pubkey_offchain_tx(
    tree: &mut CircuitAccountTree,
    change_pubkey_offchain: &ChangePubKeyOp,
) -> ChangePubkeyOffChainWitness<Bn256> {
    let change_pubkey_data = ChangePubkeyOffChainData {
        account_id: change_pubkey_offchain.account_id,
        address: eth_address_to_fr(&change_pubkey_offchain.tx.account),
        new_pubkey_hash: change_pubkey_offchain.tx.new_pk_hash.to_fr(),
        nonce: Fr::from_str(&change_pubkey_offchain.tx.nonce.to_string()).unwrap(),
    };

    apply_change_pubkey_offchain(tree, change_pubkey_data)
}

pub fn apply_change_pubkey_offchain(
    tree: &mut CircuitAccountTree,
    change_pubkey_offcahin: ChangePubkeyOffChainData,
) -> ChangePubkeyOffChainWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    debug!("Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, change_pubkey_offcahin.account_id, 0);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_id_fe = Fr::from_str(&change_pubkey_offcahin.account_id.to_string()).unwrap();
    //calculate a and b
    let a = Fr::zero();
    let b = Fr::zero();

    //applying deposit
    let (account_witness_before, account_witness_after, balance_before, balance_after) =
        apply_leaf_operation(
            tree,
            change_pubkey_offcahin.account_id,
            0,
            |acc| {
                assert_eq!(
                    acc.address, change_pubkey_offcahin.address,
                    "change pubkey address tx mismatch"
                );
                acc.pub_key_hash = change_pubkey_offcahin.new_pubkey_hash;
                acc.nonce.add_assign(&Fr::from_str("1").unwrap());
            },
            |_| {},
        );

    let after_root = tree.root_hash();
    debug!("After root = {}", after_root);
    let (audit_path_after, audit_balance_path_after) =
        get_audits(tree, change_pubkey_offcahin.account_id, 0);

    ChangePubkeyOffChainWitness {
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
            eth_address: Some(change_pubkey_offcahin.address),
            amount_packed: Some(Fr::zero()),
            full_amount: Some(Fr::zero()),
            fee: Some(Fr::zero()),
            a: Some(a),
            b: Some(b),
            pub_nonce: Some(change_pubkey_offcahin.nonce),
            new_pub_key_hash: Some(change_pubkey_offcahin.new_pubkey_hash),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("7").unwrap()),
    }
}

pub fn calculate_change_pubkey_offchain_from_witness(
    change_pubkey_offchain_witness: &ChangePubkeyOffChainWitness<Bn256>,
) -> Vec<Operation<Bn256>> {
    change_pubkey_offchain_witness
        .get_pubdata()
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .enumerate()
        .map(|(chunk_n, pubdata_chunk)| Operation {
            new_root: change_pubkey_offchain_witness.after_root,
            tx_type: change_pubkey_offchain_witness.tx_type,
            chunk: Some(Fr::from_str(&chunk_n.to_string()).unwrap()),
            pubdata_chunk: Some(pubdata_chunk),
            first_sig_msg: Some(Fr::zero()),
            second_sig_msg: Some(Fr::zero()),
            third_sig_msg: Some(Fr::zero()),
            signature_data: SignatureData::init_empty(),
            signer_pub_key_packed: vec![Some(false); 256],
            args: change_pubkey_offchain_witness.args.clone(),
            lhs: change_pubkey_offchain_witness.before.clone(),
            rhs: change_pubkey_offchain_witness.after.clone(),
        })
        .collect()
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::witness::test_utils::{check_circuit, test_genesis_plasma_state};
    use models::node::Account;
    use models::primitives::pack_bits_into_bytes_in_order;
    use testkit::zksync_account::ZksyncAccount;

    #[test]
    #[ignore]
    fn test_change_pubkey_offchain_success() {
        let change_pkhash_to_account_id = 0xc1;
        let zksync_account = ZksyncAccount::rand();
        zksync_account.set_account_id(Some(change_pkhash_to_account_id));
        let change_pkhash_to_account_address = zksync_account.address;
        let (mut plasma_state, mut circuit_account_tree) = test_genesis_plasma_state(vec![(
            change_pkhash_to_account_id,
            Account::default_with_address(&change_pkhash_to_account_address),
        )]);

        let fee_account_id = 0;
        let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

        let change_pkhash_op = ChangePubKeyOp {
            tx: zksync_account.create_change_pubkey_tx(None, true, false),
            account_id: change_pkhash_to_account_id,
        };

        println!("node root hash before op: {:?}", plasma_state.root_hash());
        plasma_state
            .apply_change_pubkey_op(&change_pkhash_op)
            .expect("applying op fail");
        println!("node root hash after op: {:?}", plasma_state.root_hash());
        println!(
            "node pubdata: {}",
            hex::encode(&change_pkhash_op.get_public_data())
        );

        let change_pkhash_witness =
            apply_change_pubkey_offchain_tx(&mut witness_accum.account_tree, &change_pkhash_op);
        let change_pkhash_operations =
            calculate_change_pubkey_offchain_from_witness(&change_pkhash_witness);
        let pub_data_from_witness = change_pkhash_witness.get_pubdata();

        //        println!("Change pk onchain witness: {:#?}", change_pkhash_witness);

        assert_eq!(
            hex::encode(pack_bits_into_bytes_in_order(pub_data_from_witness.clone())),
            hex::encode(change_pkhash_op.get_public_data()),
            "pubdata from witness incorrect"
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
