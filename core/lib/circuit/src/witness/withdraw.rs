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
        account_tree_depth, ACCOUNT_ID_BIT_WIDTH, AMOUNT_EXPONENT_BIT_WIDTH,
        AMOUNT_MANTISSA_BIT_WIDTH, BALANCE_BIT_WIDTH, CHUNK_BIT_WIDTH, ETH_ADDRESS_BIT_WIDTH,
        FEE_EXPONENT_BIT_WIDTH, FEE_MANTISSA_BIT_WIDTH, NEW_PUBKEY_HASH_WIDTH, NONCE_BIT_WIDTH,
        TOKEN_BIT_WIDTH, TX_TYPE_BIT_WIDTH,
    },
    primitives::FloatConversions,
};
use zksync_types::operations::WithdrawOp;
// Local deps
use crate::{
    operation::{Operation, OperationArguments, OperationBranch, OperationBranchWitness},
    utils::resize_grow_only,
    witness::{
        utils::{apply_leaf_operation, get_audits, SigDataInput},
        Witness,
    },
};

pub struct WithdrawData {
    pub amount: u128,
    pub fee: u128,
    pub token: u32,
    pub account_address: u32,
    pub eth_address: Fr,
}

pub struct WithdrawWitness<E: RescueEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}

impl Witness for WithdrawWitness<Bn256> {
    type OperationType = WithdrawOp;
    type CalculateOpsInput = SigDataInput;

    fn apply_tx(tree: &mut CircuitAccountTree, withdraw: &WithdrawOp) -> Self {
        let withdraw_data = WithdrawData {
            amount: withdraw.tx.amount.to_u128().unwrap(),
            fee: withdraw.tx.fee.to_u128().unwrap(),
            token: u32::from(withdraw.tx.token),
            account_address: withdraw.account_id,
            eth_address: eth_address_to_fr(&withdraw.tx.to),
        };
        // le_bit_vector_into_field_element()
        Self::apply_data(tree, &withdraw_data)
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
            WithdrawOp::CHUNKS * CHUNK_BIT_WIDTH,
            false,
        );
        pubdata_bits
    }

    fn calculate_operations(&self, input: SigDataInput) -> Vec<Operation<Bn256>> {
        let pubdata_chunks: Vec<_> = self
            .get_pubdata()
            .chunks(CHUNK_BIT_WIDTH)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect();

        let operation_zero = Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("0").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[0]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.before.clone(),
            rhs: self.before.clone(),
        };

        let rest_operations = (1..WithdrawOp::CHUNKS).map(|chunk| Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str(&chunk.to_string()).unwrap()),
            pubdata_chunk: Some(pubdata_chunks[chunk]),
            first_sig_msg: Some(input.first_sig_msg),
            second_sig_msg: Some(input.second_sig_msg),
            third_sig_msg: Some(input.third_sig_msg),
            signature_data: input.signature.clone(),
            signer_pub_key_packed: input.signer_pub_key_packed.to_vec(),
            args: self.args.clone(),
            lhs: self.after.clone(),
            rhs: self.after.clone(),
        });
        std::iter::once(operation_zero)
            .chain(rest_operations)
            .collect()
    }
}

impl<E: RescueEngine> WithdrawWitness<E> {
    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("3").unwrap(), //Corresponding tx_type
            TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.pub_key_hash.unwrap(),
            NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.eth_address.unwrap(),
            ETH_ADDRESS_BIT_WIDTH,
        );
        append_be_fixed_width(&mut sig_bits, &self.before.token.unwrap(), TOKEN_BIT_WIDTH);
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.full_amount.unwrap(),
            BALANCE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.fee.unwrap(),
            FEE_MANTISSA_BIT_WIDTH + FEE_EXPONENT_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}

impl WithdrawWitness<Bn256> {
    fn apply_data(tree: &mut CircuitAccountTree, withdraw: &WithdrawData) -> Self {
        //preparing data and base witness
        let before_root = tree.root_hash();
        log::debug!("Initial root = {}", before_root);
        let (audit_path_before, audit_balance_path_before) =
            get_audits(tree, withdraw.account_address, withdraw.token);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << account_tree_depth());
        let account_address_fe = Fr::from_str(&withdraw.account_address.to_string()).unwrap();
        let token_fe = Fr::from_str(&withdraw.token.to_string()).unwrap();
        let amount_as_field_element = Fr::from_str(&withdraw.amount.to_string()).unwrap();

        let amount_bits = FloatConversions::to_float(
            withdraw.amount,
            AMOUNT_EXPONENT_BIT_WIDTH,
            AMOUNT_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let amount_encoded: Fr = le_bit_vector_into_field_element(&amount_bits);

        let fee_as_field_element = Fr::from_str(&withdraw.fee.to_string()).unwrap();

        let fee_bits = FloatConversions::to_float(
            withdraw.fee,
            FEE_EXPONENT_BIT_WIDTH,
            FEE_MANTISSA_BIT_WIDTH,
            10,
        )
        .unwrap();

        let fee_encoded: Fr = le_bit_vector_into_field_element(&fee_bits);

        //calculate a and b

        //applying withdraw

        let (account_witness_before, account_witness_after, balance_before, balance_after) =
            apply_leaf_operation(
                tree,
                withdraw.account_address,
                withdraw.token,
                |acc| {
                    acc.nonce.add_assign(&Fr::from_str("1").unwrap());
                },
                |bal| {
                    bal.value.sub_assign(&amount_as_field_element);
                    bal.value.sub_assign(&fee_as_field_element);
                },
            );

        let after_root = tree.root_hash();
        log::debug!("After root = {}", after_root);
        let (audit_path_after, audit_balance_path_after) =
            get_audits(tree, withdraw.account_address, withdraw.token);

        let a = balance_before;
        let mut b = amount_as_field_element;
        b.add_assign(&fee_as_field_element);

        WithdrawWitness {
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
                eth_address: Some(withdraw.eth_address),
                amount_packed: Some(amount_encoded),
                full_amount: Some(amount_as_field_element),
                fee: Some(fee_encoded),
                pub_nonce: Some(Fr::zero()),
                a: Some(a),
                b: Some(b),
                new_pub_key_hash: Some(Fr::zero()),
            },
            before_root: Some(before_root),
            after_root: Some(after_root),
            tx_type: Some(Fr::from_str("3").unwrap()),
        }
    }
}
