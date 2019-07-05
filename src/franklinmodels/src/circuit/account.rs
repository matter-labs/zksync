use crate::params;

use ff::Field;
use franklin_crypto::alt_babyjubjub::JubjubEngine;

use models::primitives::{GetBits, GetBitsFixed};
#[derive(Debug, Clone)]
pub struct CircuitAccount<E: JubjubEngine> {
    pub subtree_root_hash: E::Fr,
    pub nonce: E::Fr,
    pub pub_x: E::Fr,
    pub pub_y: E::Fr,
}

// impl<'a, E: JubjubEngine> std::default::Default for CircuitAccount<'a, E> {
//     //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
//     fn default() -> Self {
//         // let balance=SparseMerkleTree<Balance<E>, E::Fr, PedersenHasher<E>>::new();
//         let balance_tree = CircuitBalanceTree::new(*params::BALANCE_TREE_DEPTH as u32);
//         let subaccount_tree = CircuitSubaccountTree::new(*params::SUBACCOUNT_TREE_DEPTH as u32);
//         let balance_root = balance_tree.root_hash();
//         let subaccount_root = subaccount_tree.root_hash();
//         let hasher = PedersenHasher::<E>::default();
//         Self {
//             subtree_root_hash: E::Fr::zero(),
//             nonce: E::Fr::zero(),
//             pub_x: E::Fr::zero(),
//             pub_y: E::Fr::zero(),
//         }
//     }
// }

impl<E: JubjubEngine> GetBits for CircuitAccount<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        //TODO: verify_order

        leaf_content.extend(self.nonce.get_bits_le_fixed(params::NONCE_BIT_WIDTH));
        leaf_content.extend(self.pub_y.get_bits_le_fixed(params::FR_BIT_WIDTH - 1));
        leaf_content.extend(self.pub_x.get_bits_le_fixed(1));
        leaf_content.extend(
            self.subtree_root_hash
                .get_bits_le_fixed(params::FR_BIT_WIDTH),
        );
        println!("test acc len {}", leaf_content.len());

        leaf_content
    }
}

//TODO: probably simpler declaration
pub struct Balance<E: JubjubEngine> {
    pub value: E::Fr,
}

impl<E: JubjubEngine> GetBits for Balance<E> {
    fn get_bits_le(&self) -> Vec<bool> {
        let mut leaf_content = Vec::new();
        leaf_content.extend(self.value.get_bits_le_fixed(params::BALANCE_BIT_WIDTH));

        leaf_content
    }
}

impl<E: JubjubEngine> std::default::Default for Balance<E> {
    //default should be changed: since subtree_root_hash is not zero for all zero balances and subaccounts
    fn default() -> Self {
        Self {
            value: E::Fr::zero(),
        }
    }
}
