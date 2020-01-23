use super::utils::*;

use crate::operation::SignatureData;
use crate::operation::*;
use ff::{Field, PrimeField};
use franklin_crypto::jubjub::JubjubEngine;
use models::circuit::account::CircuitAccountTree;
use models::circuit::utils::{append_be_fixed_width, le_bit_vector_into_field_element};
use models::node::FullExitOp;
use models::params as franklin_constants;
use pairing::bn256::*;
pub struct FullExitData {
    pub token: u32,
    pub account_address: u32,
    pub ethereum_key: Fr,
    pub pub_nonce: Fr,
}
pub struct FullExitWitness<E: JubjubEngine> {
    pub before: OperationBranch<E>,
    pub after: OperationBranch<E>,
    pub args: OperationArguments<E>,
    pub before_root: Option<E::Fr>,
    pub after_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
}
impl<E: JubjubEngine> FullExitWitness<E> {
    pub fn get_pubdata(&self, sig_data: &SignatureData, signer_pubkey: &[bool]) -> Vec<bool> {
        assert_eq!(signer_pubkey.len(), 256);
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
        pubdata_bits.extend(signer_pubkey.to_vec());

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.ethereum_key.unwrap(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.pub_nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );

        pubdata_bits.extend(sig_data.r_packed.iter().map(|x| x.unwrap()));
        pubdata_bits.extend(sig_data.s.iter().map(|x| x.unwrap()));

        append_be_fixed_width(
            &mut pubdata_bits,
            &self.args.full_amount.unwrap(),
            franklin_constants::BALANCE_BIT_WIDTH,
        );

        pubdata_bits.resize(18 * franklin_constants::CHUNK_BIT_WIDTH, false);
        // println!("pub_data outside: ");
        // for (i, bit) in pubdata_bits.iter().enumerate() {
        //     if i % 64 == 0 {
        //         println!("")
        //     } else if i % 8 == 0 {
        //         print!(" ")
        //     };
        //     let numb = {
        //         if *bit {
        //             1
        //         } else {
        //             0
        //         }
        //     };
        //     print!("{}", numb);
        // }
        // println!("");
        pubdata_bits
    }

    pub fn get_sig_bits(&self) -> Vec<bool> {
        let mut sig_bits = vec![];
        append_be_fixed_width(
            &mut sig_bits,
            &Fr::from_str("6").unwrap(), //Corresponding tx_type
            franklin_constants::TX_TYPE_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.pub_key_hash.unwrap(),
            franklin_constants::NEW_PUBKEY_HASH_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.args.ethereum_key.unwrap(),
            franklin_constants::ETHEREUM_KEY_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.token.unwrap(),
            franklin_constants::TOKEN_BIT_WIDTH,
        );
        append_be_fixed_width(
            &mut sig_bits,
            &self.before.witness.account_witness.nonce.unwrap(),
            franklin_constants::NONCE_BIT_WIDTH,
        );
        sig_bits
    }
}
pub fn apply_full_exit_tx(
    tree: &mut CircuitAccountTree,
    full_exit: &FullExitOp,
    is_success: bool,
) -> FullExitWitness<Bn256> {
    let full_exit = FullExitData {
        token: u32::from(full_exit.priority_op.token),
        account_address: full_exit.priority_op.account_id,
        ethereum_key: Fr::from_hex(&format!("{:x}", &full_exit.priority_op.eth_address)).unwrap(),
        pub_nonce: Fr::from_str(&full_exit.priority_op.nonce.to_string()).unwrap(),
    };
    // le_bit_vector_into_field_element()
    apply_full_exit(tree, &full_exit, is_success)
}
pub fn apply_full_exit(
    tree: &mut CircuitAccountTree,
    full_exit: &FullExitData,
    is_success: bool,
) -> FullExitWitness<Bn256> {
    //preparing data and base witness
    let before_root = tree.root_hash();
    println!("Initial root = {}", before_root);
    let (audit_path_before, audit_balance_path_before) =
        get_audits(tree, full_exit.account_address, full_exit.token);

    let capacity = tree.capacity();
    assert_eq!(capacity, 1 << franklin_constants::account_tree_depth());
    let account_address_fe = Fr::from_str(&full_exit.account_address.to_string()).unwrap();
    let token_fe = Fr::from_str(&full_exit.token.to_string()).unwrap();

    //applying full_exit
    let amount_to_exit = {
        let (_, _, balance, _) = apply_leaf_operation(
            tree,
            full_exit.account_address,
            full_exit.token,
            |_| {},
            |_| {},
        );
        if is_success {
            balance
        } else {
            Fr::zero()
        }
    };

    let (account_witness_before, account_witness_after, balance_before, balance_after) = {
        if is_success {
            apply_leaf_operation(
                tree,
                full_exit.account_address,
                full_exit.token,
                |acc| {
                    acc.nonce.add_assign(&Fr::from_str("1").unwrap());
                },
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
    println!("After root = {}", after_root);
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
            ethereum_key: Some(full_exit.ethereum_key),
            amount_packed: Some(Fr::zero()),
            full_amount: Some(amount_to_exit),
            fee: Some(Fr::zero()),
            pub_nonce: Some(full_exit.pub_nonce),
            a: Some(a),
            b: Some(b),
            new_pub_key_hash: Some(Fr::zero()),
        },
        before_root: Some(before_root),
        after_root: Some(after_root),
        tx_type: Some(Fr::from_str("6").unwrap()),
    }
}
pub fn calculate_full_exit_operations_from_witness(
    full_exit_witness: &FullExitWitness<Bn256>,
    first_sig_msg: &Fr,
    second_sig_msg: &Fr,
    third_sig_msg: &Fr,
    signature_data: &SignatureData,
    signer_pub_key_packed: &[Option<bool>],
) -> Vec<Operation<Bn256>> {
    let signer_pub_key_bits: Vec<bool> = signer_pub_key_packed
        .to_vec()
        .iter()
        .map(|x| x.unwrap())
        .collect();
    let pubdata_chunks: Vec<_> = full_exit_witness
        .get_pubdata(signature_data, &signer_pub_key_bits)
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();

    let mut operations = vec![];
    operations.push(Operation {
        new_root: full_exit_witness.after_root,
        tx_type: full_exit_witness.tx_type,
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(*first_sig_msg),
        second_sig_msg: Some(*second_sig_msg),
        third_sig_msg: Some(*third_sig_msg),
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),
        args: full_exit_witness.args.clone(),
        lhs: full_exit_witness.before.clone(),
        rhs: full_exit_witness.before.clone(),
        signature_data: signature_data.clone(),
    });

    for (i, pubdata_chunk) in pubdata_chunks.iter().cloned().enumerate().take(18).skip(1) {
        operations.push(Operation {
            new_root: full_exit_witness.after_root,
            tx_type: full_exit_witness.tx_type,
            chunk: Some(Fr::from_str(&i.to_string()).unwrap()),
            pubdata_chunk: Some(pubdata_chunk),
            first_sig_msg: Some(*first_sig_msg),
            second_sig_msg: Some(*second_sig_msg),
            third_sig_msg: Some(*third_sig_msg),
            signer_pub_key_packed: signer_pub_key_packed.to_vec(),
            args: full_exit_witness.args.clone(),
            lhs: full_exit_witness.after.clone(),
            rhs: full_exit_witness.after.clone(),
            signature_data: signature_data.clone(),
        });
    }

    operations
}
#[cfg(test)]
mod test {
    #[test]
    #[ignore]
    fn test_full_exit_success() {
        //TODO: Full exit test are disabled
        // 1) They don't work anyway
        // 2) Full exit will be simplified.
    }

    #[test]
    #[ignore]
    fn test_full_exit_failure() {
        //TODO: Full exit test are disabled
        // 1) They don't work anyway
        // 2) Full exit will be simplified.
    }
}
