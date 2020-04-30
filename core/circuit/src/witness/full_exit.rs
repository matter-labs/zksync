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
    node::operations::FullExitOp,
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
            token: u32::from(full_exit.priority_op.token),
            account_address: full_exit.priority_op.account_id,
            eth_address: eth_address_to_fr(&full_exit.priority_op.eth_address),
            full_exit_amount: full_exit
                .withdraw_amount
                .clone()
                .map(|amount| Fr::from_str(&amount.to_string()).unwrap())
                .unwrap_or_else(Fr::zero),
        };

        // le_bit_vector_into_field_element()
        Self::apply_data(tree, &full_exit, *is_success)
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
            &self.args.eth_address.unwrap(),
            franklin_constants::ETH_ADDRESS_BIT_WIDTH,
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

        pubdata_bits.resize(6 * franklin_constants::CHUNK_BIT_WIDTH, false);
        pubdata_bits
    }

    fn calculate_operations(&self, _input: ()) -> Vec<Operation<Bn256>> {
        let pubdata_chunks = self
            .get_pubdata()
            .chunks(64)
            .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
            .collect::<Vec<_>>();

        let empty_sig_data = SignatureData {
            r_packed: vec![Some(false); 256],
            s: vec![Some(false); 256],
        };
        let mut operations = vec![];
        operations.push(Operation {
            new_root: self.after_root,
            tx_type: self.tx_type,
            chunk: Some(Fr::from_str("0").unwrap()),
            pubdata_chunk: Some(pubdata_chunks[0]),
            first_sig_msg: Some(Fr::zero()),
            second_sig_msg: Some(Fr::zero()),
            third_sig_msg: Some(Fr::zero()),
            signer_pub_key_packed: vec![Some(false); 256],
            args: self.args.clone(),
            lhs: self.before.clone(),
            rhs: self.before.clone(),
            signature_data: empty_sig_data.clone(),
        });

        for (i, pubdata_chunk) in pubdata_chunks.iter().cloned().enumerate().take(6).skip(1) {
            operations.push(Operation {
                new_root: self.after_root,
                tx_type: self.tx_type,
                chunk: Some(Fr::from_str(&i.to_string()).unwrap()),
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
        debug!("Initial root = {}", before_root);
        let (audit_path_before, audit_balance_path_before) =
            get_audits(tree, full_exit.account_address, full_exit.token);

        let capacity = tree.capacity();
        assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
        let account_address_fe = Fr::from_str(&full_exit.account_address.to_string()).unwrap();
        let token_fe = Fr::from_str(&full_exit.token.to_string()).unwrap();

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
        debug!("After root = {}", after_root);
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
                amount_packed: Some(Fr::zero()),
                full_amount: Some(full_exit.full_exit_amount),
                fee: Some(Fr::zero()),
                pub_nonce: Some(Fr::zero()),
                a: Some(a),
                b: Some(b),
                new_pub_key_hash: Some(Fr::zero()),
            },
            before_root: Some(before_root),
            after_root: Some(after_root),
            tx_type: Some(Fr::from_str("6").unwrap()),
        }
    }
}
//         let full_exit_witness =
//             apply_full_exit_tx(&mut witness_accum.account_tree, &full_exit_op, false);
//         let full_exit_operations = calculate_full_exit_operations_from_witness(&full_exit_witness);
//         let pubdata_from_witness = full_exit_witness.get_pubdata();

//         witness_accum.add_operation_with_pubdata(full_exit_operations, pubdata_from_witness);
//         witness_accum.collect_fees(&[]);
//         witness_accum.calculate_pubdata_commitment();

//         assert_eq!(
//             plasma_state.root_hash(),
//             witness_accum
//                 .root_after_fees
//                 .expect("witness accum after root hash empty"),
//             "root hash in state keeper and witness generation code mismatch"
//         );

//         check_circuit(witness_accum.into_circuit_instance());
//     }

//     #[test]
//     #[ignore]
//     #[should_panic(expected = "chunk number 0/execute_op/op_valid")]
//     fn test_full_exit_success_but_with_wrong_amount_in_pubdata_panic() {
//         let zksync_account = ZksyncAccount::rand();
//         let account_id = 1;
//         let account_address = zksync_account.address;
//         let account = {
//             let mut account = Account::default_with_address(&account_address);
//             account.add_balance(0, &BigDecimal::from(10));
//             account.pub_key_hash = zksync_account.pubkey_hash;
//             account
//         };

//         let (mut plasma_state, mut circuit_account_tree) =
//             test_genesis_plasma_state(vec![(account_id, account)]);
//         let fee_account_id = 0;
//         let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

//         let mut full_exit_op = FullExitOp {
//             priority_op: FullExit {
//                 account_id,
//                 eth_address: account_address,
//                 token: 0,
//             },
//             withdraw_amount: Some(BigDecimal::from(10)),
//         };

//         println!("node root hash before op: {:?}", plasma_state.root_hash());
//         plasma_state.apply_full_exit_op(&full_exit_op);
//         println!("node root hash after op: {:?}", plasma_state.root_hash());

//         // here we try to withdraw more funds
//         full_exit_op.withdraw_amount = Some(BigDecimal::from(20));

//         let full_exit_witness =
//             apply_full_exit_tx(&mut witness_accum.account_tree, &full_exit_op, true);
//         let full_exit_operations = calculate_full_exit_operations_from_witness(&full_exit_witness);
//         let pubdata_from_witness = full_exit_witness.get_pubdata();

//         witness_accum.add_operation_with_pubdata(full_exit_operations, pubdata_from_witness);
//         witness_accum.collect_fees(&[]);
//         witness_accum.calculate_pubdata_commitment();

//         assert_eq!(
//             plasma_state.root_hash(),
//             witness_accum
//                 .root_after_fees
//                 .expect("witness accum after root hash empty"),
//             "root hash in state keeper and witness generation code mismatch"
//         );

//         check_circuit(witness_accum.into_circuit_instance());
//     }

//     #[test]
//     #[ignore]
//     #[should_panic(expected = "chunk number 0/execute_op/op_valid")]
//     fn test_full_exit_failure_but_with_wrong_amount_in_pubdata_panic() {
//         let zksync_account = ZksyncAccount::rand();
//         let account_id = 1;
//         let account_address = zksync_account.address;

//         let (mut plasma_state, mut circuit_account_tree) = test_genesis_plasma_state(Vec::new());
//         let fee_account_id = 0;
//         let mut witness_accum = WitnessBuilder::new(&mut circuit_account_tree, fee_account_id, 1);

//         let mut full_exit_op = FullExitOp {
//             priority_op: FullExit {
//                 account_id,
//                 eth_address: account_address,
//                 token: 0,
//             },
//             withdraw_amount: None,
//         };

//         println!("node root hash before op: {:?}", plasma_state.root_hash());
//         plasma_state.apply_full_exit_op(&full_exit_op);
//         println!("node root hash after op: {:?}", plasma_state.root_hash());

//         full_exit_op.withdraw_amount = Some(10.into());
// }
