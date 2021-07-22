// External deps
use zksync_crypto::franklin_crypto::{
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::Field,
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
        account_tree_depth, ACCOUNT_ID_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BIT_WIDTH,
        ETH_ADDRESS_BIT_WIDTH, TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
    },
};
use zksync_types::operations::DepositOp;
// Local deps
use crate::{
    operation::{
        Operation, OperationArguments, OperationBranch, OperationBranchWitness, SignatureData,
    },
    utils::resize_grow_only,
    witness::{
        utils::{apply_leaf_operation, fr_from, get_audits},
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
            token: *deposit.priority_op.token as u32,
            account_address: *deposit.account_id,
            address: eth_address_to_fr(&deposit.priority_op.to),
        };
        Self::apply_data(tree, &deposit_data)
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
            &self.before.token.unwrap(),
            TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            BALANCE_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );
        resize_grow_only(
            &mut pubdata_bits,
            DepositOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn get_offset_commitment_data(&self) -> Vec<bool> {
        let mut commitment = vec![false; DepositOp::CHUNKS * 8];
        commitment[7] = true;
        commitment
    }

    fn calculate_operations(&self, _input: ()) -> Vec<Operation<Bn256>> {
        let first_sig_msg = &Fr::zero();
        let second_sig_msg = &Fr::zero();
        let third_sig_msg = &Fr::zero();
        let signature_data = &SignatureData::init_empty();
        let signer_pub_key_packed = &[Some(false); 256]; //doesn't matter for deposit
        let pubdata_chunks: Vec<_> = self
            .get_pubdata()
            .chunks(CHUNK_BIT_WIDTH)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect();

        vlog::debug!(
            "acc_path {} \n bal_path {} ",
            self.before.witness.account_path.len(),
            self.before.witness.balance_subtree_path.len()
        );
        let operation_zero = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(0)),
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

        let rest_operations = (1..DepositOp::CHUNKS).map(|chunk| Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(chunk)),
            pubdata_chunk: Some(pubdata_chunks[chunk]),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signature_data: signature_data.clone(),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
        });
        std::iter::once(operation_zero)
            .chain(rest_operations)
            .collect()
    }
}

impl DepositWitness<Bn256> {
    fn apply_data(tree: &mut CircuitAccountTree, deposit: &DepositData) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        vlog::debug!("deposit Initial root = {}", before_root);
        let (audit_path_before, audit_balance_path_before) =
            get_audits(tree, deposit.account_address, deposit.token);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());
        let account_address_fe = fr_from(deposit.account_address);
        let token_fe = fr_from(deposit.token);
        let amount_as_field_element = fr_from(deposit.amount);
        vlog::debug!("amount_as_field_element is: {}", amount_as_field_element);
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
        vlog::debug!("deposit After root = {}", after_root);
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
                full_amount: Some(amount_as_field_element),
                a: Some(a),
                b: Some(b),
                ..Default::default()
            },
            before_root: Some(before_root),
            after_root: Some(after_root),
            tx_type: Some(fr_from(DepositOp::OP_CODE)),
        }
    }
}
