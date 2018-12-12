use ff::{Field, PrimeField};
use pairing::bn256::{Bn256, Fr};
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use crate::primitives::{field_element_to_u32, field_element_to_u128};
use crate::models::*;

type CurveUsed = Bn256;
type FrUsed = Fr;

pub type Account = account::Account<CurveUsed>;
pub type AccountTree = SparseMerkleTree<Account, FrUsed, PedersenHasher<CurveUsed>>;
pub type Tx = tx::Tx<CurveUsed>;
pub type Block = block::Block<CurveUsed>;
pub type TransactionSignature = tx::TransactionSignature<CurveUsed>;

pub struct PlasmaState {

    /// Accounts stored in a sparse Merkle tree
    pub balance_tree: AccountTree,

    /// Current block number
    pub block_number: u32,
    
}

impl PlasmaState {
    
    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.balance_tree.items.iter().map(|a| (*a.0 as u32, a.1.clone()) ).collect()
    }

    pub fn root_hash (&self) -> Fr {
        self.balance_tree.root_hash().clone()
    }

    pub fn apply(&mut self, transaction: tx::TxUnpacked) -> Result<(), ()> {

        let mut from = self.balance_tree.items.get(&transaction.from).ok_or(())?.clone();
        if field_element_to_u128(from.balance) < transaction.amount { return Err(()); }
        // TODO: check nonce: assert field_element_to_u32(from.nonce) == transaction.nonce

        // update state

        let mut to = self.balance_tree.items.get(&transaction.to).ok_or(())?.clone();
        let amount = Fr::from_str(&transaction.amount.to_string()).unwrap();
        from.balance.sub_assign(&amount);
        // TODO: subtract fee
        from.nonce.add_assign(&Fr::one());  // from.nonce++
        to.balance.add_assign(&amount);     // to.balance += amount
        self.balance_tree.insert(transaction.from, from);
        self.balance_tree.insert(transaction.to, to);

        Ok(())
    }

}