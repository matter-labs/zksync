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
use zksync_types::FullExitOp;
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

pub struct FullExitData {
    pub token: u32,
    pub account_address: u32,
    pub eth_address: Fr,
    pub full_exit_amount: Fr,
}

pub struct FullExitWitness<E: RescueEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl Witness for FullExitWitness<Bn256> {
    type OperationType = (FullExitOp, bool);
    type CalculateOpsInput = ();

    fn apply_tx(
        tree: &mut CircuitAccountTree,
        (full_exit, is_success): &(FullExitOp, bool),
    ) -> Self {
        let full_exit = FullExitData {
            token: *full_exit.priority_op.token as u32,
            account_address: *full_exit.priority_op.account_id,
            eth_address: eth_address_to_fr(&full_exit.priority_op.eth_address),
            full_exit_amount: full_exit
                .withdraw_amount
                .clone()
                .map(|amount| fr_from(amount.0))
                .unwrap_or_else(Fr::zero),
        };

        // le_bit_vector_into_field_element()
        Self::apply_data(tree, &full_exit, *is_success)
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
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
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

        resize_grow_only(
            &mut pubdata_bits,
            FullExitOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn get_offset_commitment_data(&self) -> Vec<bool> {
        let mut commitment = vec![false; FullExitOp::CHUNKS * 8];
        commitment[7] = true;
        commitment
    }

    fn calculate_operations(&self, _input: ()) -> Vec<Operation<Bn256>> {
        let pubdata_chunks = self
            .get_pubdata()
            .chunks(CHUNK_BIT_WIDTH)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect::<Vec<_>>();

        let empty_sig_data = SignatureData {
            r_packed: vec![Some(false); 256],
            s: vec![Some(false); 256],
        };
        let mut operations = vec![Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(fr_from(0)),
            pubdata_chunk: Some(pubdata_chunks[0]),
            first_sig_msg: Some(Fr::zero()),
            second_sig_msg: Some(Fr::zero()),
            third_sig_msg: Some(Fr::zero()),
            signer_pub_key_packed: vec![Some(false); 256],
            args: self.args.clone(),
            lhs: self.before.clone(),
            rhs: self.before.clone(),
            signature_data: empty_sig_data.clone(),
        }];

        for (i, pubdata_chunk) in pubdata_chunks.iter().cloned().enumerate().take(6).skip(1) {
            operations.push(Operation {
                new_root: self.after_root,
                tx_type: self.tx_type,
                chunk: Some(fr_from(i)),
                pubdata_chunk: Some(pubdata_chunk),
                first_sig_msg: Some(Fr::zero()),
                second_sig_msg: Some(Fr::zero()),
                third_sig_msg: Some(Fr::zero()),
                signer_pub_key_packed: vec![Some(false); 256],
                args: self.args.clone(),
                lhs: self.after.clone(),
                rhs: self.after.clone(),
                signature_data: empty_sig_data.clone(),
            });
        }

        operations
    }
}

impl FullExitWitness<Bn256> {
    fn apply_data(
        tree: &mut CircuitAccountTree,
        full_exit: &FullExitData,
        is_success: bool,
    ) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        vlog::debug!("Initial root = {}", before_root);
        let (audit_path_before, audit_balance_path_before) =
            get_audits(tree, full_exit.account_address, full_exit.token);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());
        let account_address_fe = fr_from(full_exit.account_address);
        let token_fe = fr_from(full_exit.token);

        let (account_witness_before, account_witness_after, balance_before, balance_after) = {
            if is_success {
                apply_leaf_operation(
                    tree,
                    full_exit.account_address,
                    full_exit.token,
                    |_| {},
                    |bal| {
                        bal.value = Fr::zero();
                    },
                )
            } else {
                apply_leaf_operation(
                    tree,
                    full_exit.account_address,
                    full_exit.token,
                    |_| {},
                    |_| {},
                )
            }
        };

        let after_root = tree.root_hash();
        vlog::debug!("After root = {}", after_root);
        let (audit_path_after, audit_balance_path_after) =
            get_audits(tree, full_exit.account_address, full_exit.token);

        let a = balance_before;
        let b = Fr::zero();

        FullExitWitness {
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
                eth_address: Some(full_exit.eth_address),
                full_amount: Some(full_exit.full_exit_amount),
                a: Some(a),
                b: Some(b),
                ..Default::default()
            },
            before_root: Some(before_root),
            after_root: Some(after_root),
            tx_type: Some(fr_from(FullExitOp::OP_CODE)),
        }
    }
}
