// External deps
use num::ToPrimitive;
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
        account_tree_depth, ACCOUNT_ID_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH,
        AMOUNT_MANTISSA_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH,
        FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
    },
    primitives::FloatConversions,
};
use zksync_types::operations::ForcedExitOp;
// Local deps
use crate::{
    operation::{Operation, OperationArguments, OperationBranch, OperationBranchWitness},
    utils::resize_grow_only,
    witness::{
        utils::{apply_leaf_operation, fr_from, get_audits, SigDataInput},
        Witness,
    },
};

pub struct ForcedExitData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub initiator_account_address: u32,
    pub target_account_address: u32,
    pub target_account_eth_address: Fr,
    pub valid_from: u64,
    pub valid_until: u64,
}

pub struct ForcedExitWitness<E: RescueEngine> {
    pub initiator_before: OperationBranch<E>,
    pub initiator_intermediate: OperationBranch<E>,
    pub initiator_after: OperationBranch<E>,
    pub target_before: OperationBranch<E>,
    pub target_intermediate: OperationBranch<E>,
    pub target_after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub intermediate_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl Witness for ForcedExitWitness<Bn256> {
    type OperationType = ForcedExitOp;
    type CalculateOpsInput = SigDataInput;

    fn apply_tx(tree: &mut CircuitAccountTree, forced_exit: &ForcedExitOp) -> Self {
        let (valid_from, valid_until) = {
            let time_range = forced_exit.tx.time_range;
            (time_range.valid_from, time_range.valid_until)
        };

        let forced_exit_data = ForcedExitData {
            amount: forced_exit
                .withdraw_amount
                .clone()
                .map(|v| v.0)
                .unwrap_or_default()
                .to_u128()
                .unwrap(),
            fee: forced_exit.tx.fee.to_u128().unwrap(),
            token: *forced_exit.tx.token as u32,
            initiator_account_address: *forced_exit.tx.initiator_account_id,
            target_account_address: *forced_exit.target_account_id,
            target_account_eth_address: eth_address_to_fr(&forced_exit.tx.target),
            valid_from,
            valid_until,
        };
        Self::apply_data(tree, &forced_exit_data)
    }

    fn get_pubdata(&self) -> Vec<bool> {
        let mut pubdata_bits = vec![];
        append_be_fixed_width(&mut pubdata_bits, &self.tx_type.unwrap(), TX_TYPE_BIT_WIDTH);

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.initiator_before.address.unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.target_before.address.unwrap(),
            ACCOUNT_ID_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.initiator_before.token.unwrap(),
            TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            BALANCE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.fee.unwrap(),
            FEE_MANTISSA_BIT_WIDTH + FEE_EXPONENT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );
        resize_grow_only(
            &mut pubdata_bits,
            ForcedExitOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn get_offset_commitment_data(&self) -> Vec<bool> {
        let mut commitment = vec![false; ForcedExitOp::CHUNKS * 8];
        commitment[7] = true;
        commitment
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
            lhs: self.initiator_before.clone(),
            rhs: self.target_before.clone(),
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
            lhs: self.initiator_intermediate.clone(),
            rhs: self.target_intermediate.clone(),
        };

        let rest_operations = (2..ForcedExitOp::CHUNKS).map(|chunk| Operation {
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
            lhs: self.initiator_after.clone(),
            rhs: self.target_after.clone(),
        });
        vec![operation_zero, operation_one]
            .into_iter()
            .chain(rest_operations)
            .collect()
    }
}

impl ForcedExitWitness<Bn256> {
    fn apply_data(tree: &mut CircuitAccountTree, forced_exit: &ForcedExitData) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        vlog::debug!("Initial root = {}", before_root);
        let (audit_path_initiator_before, audit_balance_path_initiator_before) = get_audits(
            tree,
            forced_exit.initiator_account_address,
            forced_exit.token,
        );

        let (audit_path_target_before, audit_balance_path_target_before) =
            get_audits(tree, forced_exit.target_account_address, forced_exit.token);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());
        let account_address_initiator_fe = fr_from(forced_exit.initiator_account_address);
        let account_address_target_fe = fr_from(forced_exit.target_account_address);
        let token_fe = fr_from(forced_exit.token);
        let amount_as_field_element = fr_from(forced_exit.amount);

        let amount_bits = FloatConversions::to_float(
            forced_exit.amount,
            AMOUNT_EXPONENT_BIT_WIDTH,
            AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

        let fee_as_field_element = fr_from(forced_exit.fee);

        let fee_bits = FloatConversions::to_float(
            forced_exit.fee,
            FEE_EXPONENT_BIT_WIDTH,
            FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        //applying first forced_exit part
        let (
            account_witness_initiator_before,
            account_witness_initiator_intermediate,
            balance_initiator_before,
            balance_initiator_intermediate,
        ) = apply_leaf_operation(
            tree,
            forced_exit.initiator_account_address,
            forced_exit.token,
            |acc| {
                acc.nonce.add_assign(&fr_from(1));
            },
            |bal| bal.value.sub_assign(&fee_as_field_element),
        );

        let intermediate_root = tree.root_hash();
        vlog::debug!("Intermediate root = {}", intermediate_root);

        let (audit_path_initiator_intermediate, audit_balance_path_initiator_intermediate) =
            get_audits(
                tree,
                forced_exit.initiator_account_address,
                forced_exit.token,
            );

        let (audit_path_target_intermediate, audit_balance_path_target_intermediate) =
            get_audits(tree, forced_exit.target_account_address, forced_exit.token);

        let (
            account_witness_target_intermediate,
            account_witness_target_after,
            balance_target_intermediate,
            balance_target_after,
        ) = apply_leaf_operation(
            tree,
            forced_exit.target_account_address,
            forced_exit.token,
            |_| {},
            |bal| bal.value.sub_assign(&amount_as_field_element),
        );
        let after_root = tree.root_hash();
        let (audit_path_initiator_after, audit_balance_path_initiator_after) = get_audits(
            tree,
            forced_exit.initiator_account_address,
            forced_exit.token,
        );

        let (audit_path_target_after, audit_balance_path_target_after) =
            get_audits(tree, forced_exit.target_account_address, forced_exit.token);

        //calculate a and b
        let a = balance_initiator_before;
        let b = fee_as_field_element;

        ForcedExitWitness {
            initiator_before: OperationBranch {
                address: Some(account_address_initiator_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_initiator_before,
                    account_path: audit_path_initiator_before,
                    balance_value: Some(balance_initiator_before),
                    balance_subtree_path: audit_balance_path_initiator_before,
                },
            },
            initiator_intermediate: OperationBranch {
                address: Some(account_address_initiator_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_initiator_intermediate.clone(),
                    account_path: audit_path_initiator_intermediate,
                    balance_value: Some(balance_initiator_intermediate),
                    balance_subtree_path: audit_balance_path_initiator_intermediate,
                },
            },
            initiator_after: OperationBranch {
                address: Some(account_address_initiator_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_initiator_intermediate,
                    account_path: audit_path_initiator_after,
                    balance_value: Some(balance_initiator_intermediate),
                    balance_subtree_path: audit_balance_path_initiator_after,
                },
            },
            target_before: OperationBranch {
                address: Some(account_address_target_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_target_intermediate.clone(),
                    account_path: audit_path_target_before,
                    balance_value: Some(balance_target_intermediate),
                    balance_subtree_path: audit_balance_path_target_before,
                },
            },
            target_intermediate: OperationBranch {
                address: Some(account_address_target_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_target_intermediate,
                    account_path: audit_path_target_intermediate,
                    balance_value: Some(balance_target_intermediate),
                    balance_subtree_path: audit_balance_path_target_intermediate,
                },
            },
            target_after: OperationBranch {
                address: Some(account_address_target_fe),
                token: Some(token_fe),
                witness: OperationBranchWitness {
                    account_witness: account_witness_target_after,
                    account_path: audit_path_target_after,
                    balance_value: Some(balance_target_after),
                    balance_subtree_path: audit_balance_path_target_after,
                },
            },
            args: OperationArguments {
                eth_address: Some(forced_exit.target_account_eth_address),
                amount_packed: Some(amount_encoded),
                full_amount: Some(amount_as_field_element),
                fee: Some(fee_encoded),
                a: Some(a),
                b: Some(b),
                valid_from: Some(fr_from(forced_exit.valid_from)),
                valid_until: Some(fr_from(forced_exit.valid_until)),
                ..Default::default()
            },
            before_root: Some(before_root),
            intermediate_root: Some(intermediate_root),
            after_root: Some(after_root),
            tx_type: Some(fr_from(ForcedExitOp::OP_CODE)),
        }
    }
}
