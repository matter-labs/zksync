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
    node::operations::DepositOp,
    params as franklin_constants,
};
// Local deps
use crate::{
    operation::{
        Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
    },
    witness::{
        utils::{apply_leaf_operation, get_audits},
        Witness,
    },
};

pub struct DepositData {
    pub amount: u128,
    pub token: u32,
    pub account_address: u32,
    pub address: Fr,
}

pub struct DepositWitness<E: RescueEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl Witness for DepositWitness<Bn256> {
    type OperationType = DepositOp;
    type CalculateOpsInput = ();

    fn apply_tx(tree: &mut CircuitAccountTree, deposit: &DepositOp) -> Self {
        let deposit_data = DepositData {
            amount: deposit.priority_op.amount.to_string().parse().unwrap(),
            token: u32::from(deposit.priority_op.token),
            account_address: deposit.account_id,
            address: eth_address_to_fr(&deposit.priority_op.to),
        };
        Self::apply_data(tree, &deposit_data)
    }

    fn get_pubdata(&self) -> Vec<bool> {
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
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
        );
        //        assert_eq!(pubdata_bits.len(), 37 * 8);
        pubdata_bits.resize(6 * franklin_constants::CHUNK_BIT_WIDTH, false);
        pubdata_bits
    }

    fn calculate_operations(&self, _input: ()) -> Vec<Operation<Bn256>> {
        let first_sig_msg = &Fr::zero();
        let second_sig_msg = &Fr::zero();
        let third_sig_msg = &Fr::zero();
        let signature_data = &SignatureData::init_empty();
        let signer_pub_key_packed = &[Some(false); 256]; //doesn't matter for deposit
        let pubdata_chunks: Vec<_> = self
            .get_pubdata()
            .chunks(64)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect();

        debug!(
            "acc_path{} \n bal_path {} ",
            self.before.witness.account_path.len(),
            self.before.witness.balance_subtree_path.len()
        );
        let operation_zero = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("0").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[0]),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signature_data: signature_data.clone(),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.before.clone(),
            rhs: self.before.clone(),
        };

        let operation_one = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("1").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[1]),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signature_data: signature_data.clone(),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
        };

        let operation_two = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("2").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[2]),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
            signature_data: signature_data.clone(),
        };

        let operation_three = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("3").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[3]),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
            signature_data: signature_data.clone(),
        };

        let operation_four = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("4").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[4]),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
            signature_data: signature_data.clone(),
        };

        let operation_five = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("5").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[5]),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
            signature_data: signature_data.clone(),
        };
        let operations: Vec<Operation<_>> = vec![
            operation_zero,
            operation_one,
            operation_two,
            operation_three,
            operation_four,
            operation_five,
        ];
        operations
    }
}

impl<E: RescueEngine> DepositWitness<E> {
    // CLARIFY: What? Why?
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("1").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.new_pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}

impl DepositWitness<Bn256> {
    fn apply_data(tree: &mut CircuitAccountTree, deposit: &DepositData) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        debug!("deposit Initial root = {}", before_root);
        let (audit_path_before, audit_balance_path_before) =
            get_audits(tree, deposit.account_address, deposit.token);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
        let account_address_fe = Fr::from_str(&deposit.account_address.to_string()).unwrap();
        let token_fe = Fr::from_str(&deposit.token.to_string()).unwrap();
        let amount_as_field_element = Fr::from_str(&deposit.amount.to_string()).unwrap();
        debug!("amount_as_field_element is: {}", amount_as_field_element);
        //calculate a and b
        let a = amount_as_field_element;
        let b = Fr::zero();

        //applying deposit
        let (account_witness_before, account_witness_after, balance_before, balance_after) =
            apply_leaf_operation(
                tree,
                deposit.account_address,
                deposit.token,
                |acc| {
                    assert!((acc.address == deposit.address) || (acc.address == Fr::zero()));
                    acc.address = deposit.address;
                },
                |bal| bal.value.add_assign(&amount_as_field_element),
            );

        let after_root = tree.root_hash();
        debug!("deposit After root = {}", after_root);
        let (audit_path_after, audit_balance_path_after) =
            get_audits(tree, deposit.account_address, deposit.token);

        DepositWitness {
            before: OperationBranch {
                address: Some(account_address_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_before,
                    account_path: audit_path_before,
                    balance_value: Some(balance_before),
                    balance_subtree_path: audit_balance_path_before,
                },
            },
            after: OperationBranch {
                address: Some(account_address_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_after,
                    account_path: audit_path_after,
                    balance_value: Some(balance_after),
                    balance_subtree_path: audit_balance_path_after,
                },
            },
            args: OperationArguments {
                eth_address: Some(deposit.address),
                amount_packed: Some(Fr::zero()),
                full_amount: Some(amount_as_field_element),
                fee: Some(Fr::zero()),
                a: Some(a),
                b: Some(b),
                pub_nonce: Some(Fr::zero()),
                new_pub_key_hash: Some(Fr::zero()),
            },
            before_root: Some(before_root),
            after_root: Some(after_root),
            tx_type: Some(Fr::from_str("1").unwrap()),
        }
    }
}
