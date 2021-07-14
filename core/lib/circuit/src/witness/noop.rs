// External deps
use zksync_crypto::franklin_crypto::bellman::pairing::{
    bn256::{Bn256, Fr},
    ff::Field,
};
// Workspace deps
use zksync_crypto::circuit::{
    account::CircuitAccountTree, utils::le_bit_vector_into_field_element,
};
use zksync_crypto::params::CHUNK_BIT_WIDTH;
use zksync_types::operations::NoopOp;
// Local deps
use crate::{
    account::AccountWitness,
    operation::{Operation, OperationBranch, OperationBranchWitness, SignatureData},
    witness::utils::{fr_from, get_audits},
};

pub fn noop_operation(tree: &CircuitAccountTree, acc_id: u32) -> Operation<Bn256> {
    let signature_data = SignatureData::init_empty();
    let first_sig_msg = Fr::zero();
    let second_sig_msg = Fr::zero();
    let third_sig_msg = Fr::zero();
    let signer_pub_key_packed = [Some(false); 256];

    let acc = tree.get(acc_id).unwrap();
    let account_address_fe = fr_from(acc_id);
    let token_fe = Fr::zero();
    let balance_value = match acc.subtree.get(0) {
        None => Fr::zero(),
        Some(bal) => bal.value,
    };
    let pubdata = vec![false; CHUNK_BIT_WIDTH];
    let pubdata_chunks: Vec<_> = pubdata
        .chunks(CHUNK_BIT_WIDTH)
        .map(|x| le_bit_vector_into_field_element(&x.to_vec()))
        .collect();
    let (audit_account, audit_balance) = get_audits(tree, acc_id, 0);

    Operation {
        new_root: Some(tree.root_hash()),
        tx_type: Some(fr_from(NoopOp::OP_CODE)),
        chunk: Some(fr_from(0)),
        pubdata_chunk: Some(pubdata_chunks[0]),
        first_sig_msg: Some(first_sig_msg),
        second_sig_msg: Some(second_sig_msg),
        third_sig_msg: Some(third_sig_msg),
        signature_data,
        signer_pub_key_packed: signer_pub_key_packed.to_vec(),

        args: Default::default(),
        lhs: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(acc.nonce),
                    pub_key_hash: Some(acc.pub_key_hash),
                    address: Some(acc.address),
                },
                account_path: audit_account.clone(),
                balance_value: Some(balance_value),
                balance_subtree_path: audit_balance.clone(),
            },
        },
        rhs: OperationBranch {
            address: Some(account_address_fe),
            token: Some(token_fe),
            witness: OperationBranchWitness {
                account_witness: AccountWitness {
                    nonce: Some(acc.nonce),
                    pub_key_hash: Some(acc.pub_key_hash),
                    address: Some(acc.address),
                },
                account_path: audit_account,
                balance_value: Some(balance_value),
                balance_subtree_path: audit_balance,
            },
        },
    }
}
