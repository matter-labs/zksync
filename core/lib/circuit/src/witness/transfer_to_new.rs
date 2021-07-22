// Workspace deps
use zksync_crypto::franklin_crypto::{
    bellman::pairing::{
        bn256::{Bn256, Fr},
        ff::Field,
    },
    rescue::RescueEngine,
};
use zksync_crypto::{
    circuit::{
        account::CircuitAccountTree,
        utils::{append_be_fixed_width, eth_address_to_fr, le_bit_vector_into_field_element},
    },
    params::{
        account_tree_depth, ACCOUNT_ID_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH,
        AMOUNT_MANTISSA_BIT_WIDTH, CHUNK_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH, FEE_EXPONENT_BIT_WIDTH,
        FEE_MANTISSA_BIT_WIDTH, TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
    },
    primitives::FloatConversions,
};
use zksync_types::operations::TransferToNewOp;
// Local deps
use crate::{
    operation::{Operation, OperationArguments, OperationBranch, OperationBranchWitness},
    utils::resize_grow_only,
    witness::{
        utils::{apply_leaf_operation, fr_from, get_audits, SigDataInput},
        Witness,
    },
};
use zksync_types::operations::ChangePubKeyOp;

pub struct TransferToNewData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub from_account_address: u32,
    pub to_account_address: u32,
    pub new_address: Fr,
    pub valid_from: u64,
    pub valid_until: u64,
}

pub struct TransferToNewWitness<E: RescueEngine> {
    pub from_before: OperationBranch<E>,
    pub from_intermediate: OperationBranch<E>,
    pub from_after: OperationBranch<E>,
    pub to_before: OperationBranch<E>,
    pub to_intermediate: OperationBranch<E>,
    pub to_after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub intermediate_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl Witness for TransferToNewWitness<Bn256> {
    type OperationType = TransferToNewOp;
    type CalculateOpsInput = SigDataInput;

    fn apply_tx(tree: &mut CircuitAccountTree, transfer_to_new: &TransferToNewOp) -> Self {
        let time_range = transfer_to_new.tx.time_range.unwrap_or_default();
        let transfer_data = TransferToNewData {
            amount: transfer_to_new.tx.amount.to_string().parse().unwrap(),
            fee: transfer_to_new.tx.fee.to_string().parse().unwrap(),
            token: *transfer_to_new.tx.token as u32,
            from_account_address: *transfer_to_new.from,
            to_account_address: *transfer_to_new.to,
            new_address: eth_address_to_fr(&transfer_to_new.tx.to),
            valid_from: time_range.valid_from,
            valid_until: time_range.valid_until,
        };
        // le_bit_vector_into_field_element()
        Self::apply_data(tree, &transfer_data)
    }

    fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(&mut pubdata_bits, &self.tx_type.unwrap(), TX_TYPE_BIT_WIDTH);

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.address.unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.from_before.token.unwrap(),
            TOKEN_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.amount_packed.unwrap(),
            AMOUNT_MANTISSA_BIT_WIDTH + AMOUNT_EXPONENT_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.to_before.address.unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            FEE_MANTISSA_BIT_WIDTH + FEE_EXPONENT_BIT_WIDTH,
        );
        resize_grow_only(
            &mut pubdata_bits,
            TransferToNewOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn get_offset_commitment_data(&self) -> Vec<bool> {
        vec![false; TransferToNewOp::CHUNKS * 8]
    }

    fn calculate_operations(&self, input: SigDataInput) -> Vec<Operation<Bn256>> {
        let pubdata_chunks: Vec<_> = self
            .get_pubdata()
            .chunks(CHUNK_BIT_WIDTH)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect();

        let operation_zero = Operation {
            new_root: self.intermediate_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(0)),
            pubdata_chunk: Some(pubdata_chunks[0]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.from_before.clone(),
            rhs: self.to_before.clone(),
        };

        let operation_one = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(1)),
            pubdata_chunk: Some(pubdata_chunks[1]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.from_intermediate.clone(),
            rhs: self.to_intermediate.clone(),
        };

        let rest_operations = (2..ChangePubKeyOp::CHUNKS).map(|chunk| Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(chunk)),
            pubdata_chunk: Some(pubdata_chunks[chunk]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.from_after.clone(),
            rhs: self.to_after.clone(),
        });
        vec![operation_zero, operation_one]
            .into_iter()
            .chain(rest_operations)
            .collect()
    }
}

impl TransferToNewWitness<Bn256> {
    fn apply_data(tree: &mut CircuitAccountTree, transfer_to_new: &TransferToNewData) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        vlog::debug!("Initial root = {}", before_root);
        let (audit_path_from_before, audit_balance_path_from_before) = get_audits(
            tree,
            transfer_to_new.from_account_address,
            transfer_to_new.token,
        );

        let (audit_path_to_before, audit_balance_path_to_before) = get_audits(
            tree,
            transfer_to_new.to_account_address,
            transfer_to_new.token,
        );

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());
        let account_address_from_fe = fr_from(transfer_to_new.from_account_address);
        let account_address_to_fe = fr_from(transfer_to_new.to_account_address);
        let token_fe = fr_from(transfer_to_new.token);
        let amount_as_field_element = fr_from(transfer_to_new.amount);

        let amount_bits = FloatConversions::to_float(
            transfer_to_new.amount,
            AMOUNT_EXPONENT_BIT_WIDTH,
            AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

        vlog::debug!("test_transfer_to_new.fee {}", transfer_to_new.fee);
        let fee_as_field_element = fr_from(transfer_to_new.fee);
        vlog::debug!(
            "test transfer_to_new fee_as_field_element = {}",
            fee_as_field_element
        );
        let fee_bits = FloatConversions::to_float(
            transfer_to_new.fee,
            FEE_EXPONENT_BIT_WIDTH,
            FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);
        vlog::debug!("fee_encoded in test_transfer_to_new {}", fee_encoded);

        let valid_from = transfer_to_new.valid_from;
        let valid_until = transfer_to_new.valid_until;
        //applying first transfer part
        let (
            account_witness_from_before,
            account_witness_from_intermediate,
            balance_from_before,
            balance_from_intermediate,
        ) = apply_leaf_operation(
            tree,
            transfer_to_new.from_account_address,
            transfer_to_new.token,
            |acc| {
                acc.nonce.add_assign(&fr_from(1));
            },
            |bal| {
                bal.value.sub_assign(&amount_as_field_element);
                bal.value.sub_assign(&fee_as_field_element)
            },
        );

        let intermediate_root = tree.root_hash();
        vlog::debug!("Intermediate root = {}", intermediate_root);

        let (audit_path_from_intermediate, audit_balance_path_from_intermediate) = get_audits(
            tree,
            transfer_to_new.from_account_address,
            transfer_to_new.token,
        );

        let (audit_path_to_intermediate, audit_balance_path_to_intermediate) = get_audits(
            tree,
            transfer_to_new.to_account_address,
            transfer_to_new.token,
        );

        let (
            account_witness_to_intermediate,
            account_witness_to_after,
            balance_to_intermediate,
            balance_to_after,
        ) = apply_leaf_operation(
            tree,
            transfer_to_new.to_account_address,
            transfer_to_new.token,
            |acc| {
                assert!((acc.address == Fr::zero()));
                acc.address = transfer_to_new.new_address;
            },
            |bal| bal.value.add_assign(&amount_as_field_element),
        );
        let after_root = tree.root_hash();
        let (audit_path_from_after, audit_balance_path_from_after) = get_audits(
            tree,
            transfer_to_new.from_account_address,
            transfer_to_new.token,
        );

        let (audit_path_to_after, audit_balance_path_to_after) = get_audits(
            tree,
            transfer_to_new.to_account_address,
            transfer_to_new.token,
        );

        //calculate a and b
        let a = balance_from_before;
        let mut b = amount_as_field_element;
        b.add_assign(&fee_as_field_element);
        TransferToNewWitness {
            from_before: OperationBranch {
                address: Some(account_address_from_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_from_before,
                    account_path: audit_path_from_before,
                    balance_value: Some(balance_from_before),
                    balance_subtree_path: audit_balance_path_from_before,
                },
            },
            from_intermediate: OperationBranch {
                address: Some(account_address_from_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_from_intermediate.clone(),
                    account_path: audit_path_from_intermediate,
                    balance_value: Some(balance_from_intermediate),
                    balance_subtree_path: audit_balance_path_from_intermediate,
                },
            },
            from_after: OperationBranch {
                address: Some(account_address_from_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_from_intermediate,
                    account_path: audit_path_from_after,
                    balance_value: Some(balance_from_intermediate),
                    balance_subtree_path: audit_balance_path_from_after,
                },
            },
            to_before: OperationBranch {
                address: Some(account_address_to_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_to_intermediate.clone(),
                    account_path: audit_path_to_before,
                    balance_value: Some(balance_to_intermediate),
                    balance_subtree_path: audit_balance_path_to_before,
                },
            },
            to_intermediate: OperationBranch {
                address: Some(account_address_to_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_to_intermediate,
                    account_path: audit_path_to_intermediate,
                    balance_value: Some(balance_to_intermediate),
                    balance_subtree_path: audit_balance_path_to_intermediate,
                },
            },
            to_after: OperationBranch {
                address: Some(account_address_to_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_to_after,
                    account_path: audit_path_to_after,
                    balance_value: Some(balance_to_after),
                    balance_subtree_path: audit_balance_path_to_after,
                },
            },
            args: OperationArguments {
                eth_address: Some(transfer_to_new.new_address),
                amount_packed: Some(amount_encoded),
                full_amount: Some(amount_as_field_element),
                fee: Some(fee_encoded),
                a: Some(a),
                b: Some(b),
                valid_from: Some(fr_from(&valid_from)),
                valid_until: Some(fr_from(&valid_until)),
                ..Default::default()
            },
            before_root: Some(before_root),
            intermediate_root: Some(intermediate_root),
            after_root: Some(after_root),
            tx_type: Some(fr_from(TransferToNewOp::OP_CODE)),
        }
    }
}
