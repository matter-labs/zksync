use crate::account::AccountWitness;
use ff::Field;

use franklin_crypto::jubjub::{edwards, Unknown};
use franklin_crypto::jubjub::{FixedGenerators, JubjubEngine, JubjubParams};

#[derive(Clone)]
pub struct OperationBranchWitness<E: JubjubEngine> {
    pub account_witness: AccountWitness<E>,
    pub account_path: Vec<Option<E::Fr>>,

    pub balance_value: Option<E::Fr>,
    pub balance_subtree_path: Vec<Option<E::Fr>>,

    pub dummmy_subaccount_value: Option<E::Fr>,
    pub subaccount_path: Vec<Option<E::Fr>>,
}

#[derive(Clone)]
pub struct OperationBranch<E: JubjubEngine> {
    pub address: Option<E::Fr>,
    // pub signature //probably signature should be here
    pub token: Option<E::Fr>,

    pub subaccount_number: Option<E::Fr>,

    pub witness: OperationBranchWitness<E>,
}

#[derive(Clone)]
pub struct Operation<E: JubjubEngine> {
    pub new_root: Option<E::Fr>,
    pub tx_type: Option<E::Fr>,
    pub chunk: Option<E::Fr>,
    pub pubdata_chunk: Option<E::Fr>,
    pub signer_pub_key_x: Option<E::Fr>,
    pub signer_pub_key_y: Option<E::Fr>,
    pub sig_msg: Option<E::Fr>,
    pub signature: Option<TransactionSignature<E>>,
    pub args: OperationArguments<E>,
    pub lhs: OperationBranch<E>,
    pub rhs: OperationBranch<E>,
}

// impl<E: JubjubEngine> Operation<E> {
//     pub fn with_id(string_id: &str, string_tx_type: &str) -> Operation<E> {
//         Operation {
//             tx_type: E::Fr::from_str(string_tx_type),
//             chunk: E::Fr::from_str(string_id),
//             pubdata_chunk: None,
//             args: None,
//             lhs: OperationBranch {
//                 address: None,
//                 token: None,
//                 subaccount_number: None,
//                 witness: OperationBranchWitness {
//                     account_witness: AccountWitness {
//                         nonce: None,
//                         // x coordinate is supplied and parity is constrained
//                         pub_x: None,
//                         pub_y: None,
//                     },
//                     account_path: vec![],
//                     balance_value: None,
//                     balance_subtree_path: vec![],
//                     dummmy_subaccount_value: None,
//                     subaccount_path: vec![],
//                 },
//             },
//             rhs: OperationBranch {
//                 address: None,
//                 token: None,
//                 subaccount_number: None,
//                 witness: OperationBranchWitness {
//                     account_witness: AccountWitness {
//                         nonce: None,
//                         // x coordinate is supplied and parity is constrained
//                         pub_x: None,
//                         pub_y: None,
//                     },
//                     account_path: vec![],
//                     balance_value: None,
//                     balance_subtree_path: vec![],
//                     dummmy_subaccount_value: None,
//                     subaccount_path: vec![],
//                 },
//             },
//         }
//     }
// }
#[derive(Clone, Debug)]
pub struct OperationArguments<E: JubjubEngine> {
    pub a: Option<E::Fr>,
    pub b: Option<E::Fr>,
    pub amount: Option<E::Fr>,
    pub fee: Option<E::Fr>,
    pub compact_amount: Option<E::Fr>,
    pub new_pub_x: Option<E::Fr>,
    pub new_pub_y: Option<E::Fr>,
}

// use crate::models::params;
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
