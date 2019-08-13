use super::utils::*;

use crate::operation::*;
use crate::utils::*;

use ff::{BitIterator, Field, PrimeField, PrimeFieldRepr};

use crate::account::AccountWitness;
use franklin_crypto::circuit::float_point::convert_to_float;
use franklin_crypto::jubjub::JubjubEngine;
use franklinmodels::circuit::account::{
    Balance, CircuitAccount, CircuitAccountTree, CircuitBalanceTree,
};
use num_traits::cast::ToPrimitive;

use franklinmodels::node::{TransferOp, PartialExitOp};
use franklinmodels::merkle_tree::hasher::Hasher;
use franklinmodels::merkle_tree::PedersenHasher;
use franklinmodels::params as franklin_constants;
use pairing::bn256::*;



pub fn noop_operation(
    tree: &CircuitAccountTree,
    acc_id: u32, 
    sig_msg: &Fr,
    signature: Option<TransactionSignature<Bn256>>,
    signer_pub_key_x: &Fr,
    signer_pub_key_y: &Fr,
) -> Operation<Bn256> {
    let acc = tree.get(acc_id).unwrap();
    let account_address_fe = Fr::from_str(&acc_id.to_string()).unwrap();
    let token_fe = Fr::zero();
    let balance_value = match acc.subtree.get(0){
                    None => Fr::zero(),
                    Some(bal) => bal.value.clone()
                };
    let pubdata = vec![false; 64];
    let pubdata_chunks: Vec<_> = pubdata
        .chunks(64)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();
    let (audit_account, audit_balance) = get_audits(tree, acc_id, 0);

    Operation {
        new_root: Some(tree.root_hash()),
        tx_type: Some(Fr::from_str("0").unwrap()),
        chunk: Some(Fr::from_str("0").unwrap()),
        pubdata_chunk: Some(pubdata_chunks[0]),
        sig_msg: Some(sig_msg.clone()),
        signature: signature.clone(),
        signer_pub_key_x: Some(signer_pub_key_x.clone()),
        signer_pub_key_y: Some(signer_pub_key_y.clone()),

        args: OperationArguments {
            ethereum_key: Some(Fr::zero()),
            amount: Some(Fr::zero()),
            fee: Some(Fr::zero()),
            a: Some(Fr::zero()),
            b: Some(Fr::zero()),
            new_pub_key_hash: Some(Fr::zero()),
        },
        lhs: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness{
                    nonce: Some(acc.nonce.clone()),
                    pub_key_hash: Some(acc.pub_key_hash.clone()),
                },
                account_path: audit_account.clone(),
                balance_value: Some(balance_value.clone()),
                balance_subtree_path: audit_balance.clone(),
            },
        },
        rhs: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness{
                    nonce: Some(acc.nonce.clone()),
                    pub_key_hash: Some(acc.pub_key_hash.clone()),
                },
                account_path: audit_account.clone(),
                balance_value: Some(balance_value.clone()),
                balance_subtree_path: audit_balance.clone(),
            },
        },
    }
}
