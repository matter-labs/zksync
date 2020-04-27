// External deps
use crypto_exports::franklin_crypto::{
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{Field, PrimeField},
    },
    rescue::RescueEngine,
};
// Workspace deps
use models::{
    circuit::{
        account::CircuitAccountTree,
        utils::{append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element},
    },
    node::operations::ChangePubKeyOp,
    params as franklin_constants,
};
// Local deps
use crate::{
    operation::{
        Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
    },
    witness::utils::{apply_leaf_operation, get_audits},
};

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

impl ChangePubkeyOffChainWitness<Bn256> {
    pub fn apply_tx(
        tree: &mut CircuitAccountTree,
        change_pubkey_offchain: &ChangePubKeyOp,
    ) -> Self {
        let change_pubkey_data = ChangePubkeyOffChainData {
            account_id: change_pubkey_offchain.account_id,
            address: eth_address_to_fr(&change_pubkey_offchain.tx.account),
            new_pubkey_hash: change_pubkey_offchain.tx.new_pk_hash.to_fr(),
            nonce: Fr::from_str(&change_pubkey_offchain.tx.nonce.to_string()).unwrap(),
        };

        Self::apply_data(tree, change_pubkey_data)
    }

    fn apply_data(
        tree: &mut CircuitAccountTree,
        change_pubkey_offcahin: ChangePubkeyOffChainData,
    ) -> Self {
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

    pub fn calculate_operations(&self) -> Vec<Operation<Bn256>> {
        self.get_pubdata()
            .chunks(64)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .enumerate()
            .map(|(chunk_n, pubdata_chunk)| Operation {
                new_root: self.after_root,
                tx_type: self.tx_type,
                chunk: Some(Fr::from_str(&chunk_n.to_string()).unwrap()),
                pubdata_chunk: Some(pubdata_chunk),
                first_sig_msg: Some(Fr::zero()),
                second_sig_msg: Some(Fr::zero()),
                third_sig_msg: Some(Fr::zero()),
                signature_data: SignatureData::init_empty(),
                signer_pub_key_packed: vec![Some(false); 256],
                args: self.args.clone(),
                lhs: self.before.clone(),
                rhs: self.after.clone(),
            })
            .collect()
    }
}
