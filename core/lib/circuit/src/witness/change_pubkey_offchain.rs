// External deps
use num::ToPrimitive;
use zksync_crypto::franklin_crypto::{
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::{Field, PrimeField},
    },
    rescue::RescueEngine,
};
// Workspace deps
use zksync_crypto::{
    circuit::{
        account::CircuitAccountTree,
        utils::{append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element},
    },
    params::{
        account_tree_depth, ACCOUNT_ID_BIT_WIDTH, CHUNK_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH,
        FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, NEW_PUBKEY_HASH_WIDTH, NONCE_BIT_WIDTH,
        TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
    },
    primitives::FloatConversions,
};
use zksync_types::operations::ChangePubKeyOp;
// Local deps
use crate::{
    operation::{Operation, OperationArguments, OperationBranch, OperationBranchWitness},
    utils::resize_grow_only,
    witness::{
        utils::{apply_leaf_operation, get_audits, SigDataInput},
        Witness,
    },
};

pub struct ChangePubkeyOffChainData {
    pub account_id: u32,
    pub address: Fr,
    pub new_pubkey_hash: Fr,
    pub fee_token: u32,
    pub fee: u128,
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

impl Witness for ChangePubkeyOffChainWitness<Bn256> {
    type OperationType = ChangePubKeyOp;
    type CalculateOpsInput = SigDataInput;

    fn apply_tx(tree: &mut CircuitAccountTree, change_pubkey_offchain: &ChangePubKeyOp) -> Self {
        let change_pubkey_data = ChangePubkeyOffChainData {
            account_id: change_pubkey_offchain.account_id,
            address: eth_address_to_fr(&change_pubkey_offchain.tx.account),
            new_pubkey_hash: change_pubkey_offchain.tx.new_pk_hash.to_fr(),
            fee_token: u32::from(change_pubkey_offchain.tx.fee_token),
            fee: change_pubkey_offchain.tx.fee.to_u128().unwrap(),
            nonce: Fr::from_str(&change_pubkey_offchain.tx.nonce.to_string()).unwrap(),
        };

        Self::apply_data(tree, change_pubkey_data)
    }

    fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(&mut pubdata_bits, &self.tx_type.unwrap(), TX_TYPE_BIT_WIDTH);
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.address.unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.new_pub_key_hash.unwrap(),
            NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            NONCE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.token.unwrap(),
            TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            FEE_MANTISSA_BIT_WIDTH + FEE_EXPONENT_BIT_WIDTH,
        );

        resize_grow_only(
            &mut pubdata_bits,
            ChangePubKeyOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn calculate_operations(&self, input: SigDataInput) -> Vec<Operation<Bn256>> {
        self.get_pubdata()
            .chunks(CHUNK_BIT_WIDTH)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .enumerate()
            .map(|(chunk_n, pubdata_chunk)| Operation {
                new_root: self.after_root,
                tx_type: self.tx_type,
                chunk: Some(Fr::from_str(&chunk_n.to_string()).unwrap()),
                pubdata_chunk: Some(pubdata_chunk),
                first_sig_msg: Some(input.first_sig_msg),
                second_sig_msg: Some(input.second_sig_msg),
                third_sig_msg: Some(input.third_sig_msg),
                signature_data: input.signature.clone(),
                signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
                args: self.args.clone(),
                lhs: self.before.clone(),
                rhs: self.after.clone(),
            })
            .collect()
    }
}

impl ChangePubkeyOffChainWitness<Bn256> {
    fn apply_data(
        tree: &mut CircuitAccountTree,
        change_pubkey_offcahin: ChangePubkeyOffChainData,
    ) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        log::debug!("Initial root = {}", before_root);
        let (audit_path_before, audit_balance_path_before) = get_audits(
            tree,
            change_pubkey_offcahin.account_id,
            change_pubkey_offcahin.fee_token,
        );

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());
        let account_id_fe = Fr::from_str(&change_pubkey_offcahin.account_id.to_string()).unwrap();

        let fee_token_fe = Fr::from_str(&change_pubkey_offcahin.fee_token.to_string()).unwrap();
        let fee_as_field_element = Fr::from_str(&change_pubkey_offcahin.fee.to_string()).unwrap();

        let fee_bits = FloatConversions::to_float(
            change_pubkey_offcahin.fee,
            FEE_EXPONENT_BIT_WIDTH,
            FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();
        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        //applying deposit
        let (account_witness_before, account_witness_after, balance_before, balance_after) =
            apply_leaf_operation(
                tree,
                change_pubkey_offcahin.account_id,
                change_pubkey_offcahin.fee_token,
                |acc| {
                    assert_eq!(
                        acc.address, change_pubkey_offcahin.address,
                        "change pubkey address tx mismatch"
                    );
                    acc.pub_key_hash = change_pubkey_offcahin.new_pubkey_hash;
                    acc.nonce.add_assign(&Fr::from_str("1").unwrap());
                },
                |bal| {
                    bal.value.sub_assign(&fee_as_field_element);
                },
            );

        //calculate a and b
        let a = balance_before;
        let b = fee_as_field_element;

        let after_root = tree.root_hash();
        log::debug!("After root = {}", after_root);
        let (audit_path_after, audit_balance_path_after) = get_audits(
            tree,
            change_pubkey_offcahin.account_id,
            change_pubkey_offcahin.fee_token,
        );

        ChangePubkeyOffChainWitness {
            before: OperationBranch {
                address: Some(account_id_fe),
                token: Some(fee_token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_before,
                    account_path: audit_path_before,
                    balance_value: Some(balance_before),
                    balance_subtree_path: audit_balance_path_before,
                },
            },
            after: OperationBranch {
                address: Some(account_id_fe),
                token: Some(fee_token_fe),
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
                fee: Some(fee_encoded),
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
}
