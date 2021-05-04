// External
use serde::{Deserialize, Serialize};
use zksync_crypto::franklin_crypto::{
    bellman::pairing::ff::{Field, PrimeField},
    jubjub::{edwards, JubjubEngine, Unknown},
    rescue::RescueEngine,
};
// Workspace
use crate::account::AccountWitness;
use zksync_crypto::params::CONTENT_HASH_WIDTH;

#[derive(Clone, Debug)]
pub struct OperationBranchWitness<E: RescueEngine> {
    pub account_witness: AccountWitness<E>,
    pub account_path: Vec<Option<E::Fr>>,

    pub balance_value: Option<E::Fr>,
    pub balance_subtree_path: Vec<Option<E::Fr>>,
}

#[derive(Clone, Debug)]
pub struct OperationBranch<E: RescueEngine> {
    pub address: Option<E::Fr>,
    pub token: Option<E::Fr>,

    pub witness: OperationBranchWitness<E>,
}

#[derive(Clone, Debug)]
pub struct Operation<E: RescueEngine> {
    pub new_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
    pub chunk: Option<E::Fr>,
    pub pubdata_chunk: Option<E::Fr>,
    pub signer_pub_key_packed: Vec<Option<bool>>,
    pub first_sig_msg: Option<E::Fr>,
    pub second_sig_msg: Option<E::Fr>,
    pub third_sig_msg: Option<E::Fr>,
    pub signature_data: SignatureData,
    pub args: OperationArguments<E>,
    pub lhs: OperationBranch<E>,
    pub rhs: OperationBranch<E>,
}

#[derive(Clone, Debug)]
pub struct OperationArguments<E: RescueEngine> {
    pub a: Option<E::Fr>,
    pub b: Option<E::Fr>,
    pub amount_packed: Option<E::Fr>,
    pub second_amount_packed: Option<E::Fr>,
    pub special_amounts: Vec<Option<E::Fr>>,
    pub special_nonces: Vec<Option<E::Fr>>,
    pub special_tokens: Vec<Option<E::Fr>>,
    pub special_accounts: Vec<Option<E::Fr>>,
    pub special_prices: Vec<Option<E::Fr>>,
    pub special_eth_addresses: Vec<Option<E::Fr>>,
    pub special_content_hash: Vec<Option<E::Fr>>,
    pub special_serial_id: Option<E::Fr>,
    pub full_amount: Option<E::Fr>,
    pub fee: Option<E::Fr>,
    pub new_pub_key_hash: Option<E::Fr>,
    pub eth_address: Option<E::Fr>,
    pub pub_nonce: Option<E::Fr>,
    pub valid_from: Option<E::Fr>,
    pub valid_until: Option<E::Fr>,
    pub second_valid_from: Option<E::Fr>,
    pub second_valid_until: Option<E::Fr>,
}

impl<E: RescueEngine> Default for OperationArguments<E> {
    fn default() -> Self {
        OperationArguments {
            a: Some(E::Fr::zero()),
            b: Some(E::Fr::zero()),
            amount_packed: Some(E::Fr::zero()),
            second_amount_packed: Some(E::Fr::zero()),
            special_amounts: vec![Some(E::Fr::zero()); 2],
            special_nonces: vec![Some(E::Fr::zero()); 3],
            special_tokens: vec![Some(E::Fr::zero()); 3],
            special_accounts: vec![Some(E::Fr::zero()); 5],
            special_prices: vec![Some(E::Fr::zero()); 4],
            special_eth_addresses: vec![Some(E::Fr::zero()); 2],
            special_content_hash: vec![Some(E::Fr::zero()); CONTENT_HASH_WIDTH],
            special_serial_id: Some(E::Fr::zero()),
            full_amount: Some(E::Fr::zero()),
            fee: Some(E::Fr::zero()),
            new_pub_key_hash: Some(E::Fr::zero()),
            eth_address: Some(E::Fr::zero()),
            pub_nonce: Some(E::Fr::zero()),
            valid_from: Some(E::Fr::zero()),
            valid_until: Some(E::Fr::from_str(&u32::MAX.to_string()).unwrap()),
            second_valid_from: Some(E::Fr::zero()),
            second_valid_until: Some(E::Fr::from_str(&u32::MAX.to_string()).unwrap()),
        }
    }
}

#[derive(Clone)]
pub struct TransactionSignature<E: JubjubEngine> {
    pub r: edwards::Point<E, Unknown>,
    pub s: E::Fr,
}

impl<E: JubjubEngine> TransactionSignature<E> {
    pub fn empty() -> Self {
        let empty_point: edwards::Point<E, Unknown> = edwards::Point::zero();

        Self {
            r: empty_point,
            s: E::Fr::zero(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignatureData {
    pub r_packed: Vec<Option<bool>>,
    pub s: Vec<Option<bool>>,
}

impl SignatureData {
    pub fn init_empty() -> Self {
        Self {
            r_packed: vec![Some(false); 256],
            s: vec![Some(false); 256],
        }
    }
}
