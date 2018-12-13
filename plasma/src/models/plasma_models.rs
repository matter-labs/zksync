use ff::{Field, PrimeField};
use pairing::bn256;
use crate::merkle_tree::{SparseMerkleTree, PedersenHasher};
use crate::primitives::{field_element_to_u32, field_element_to_u128};
use crate::models::*;

type Engine = bn256::Bn256;
type Fr = bn256::Fr;

pub type Account = account::Account<Engine>;
pub type AccountTree = SparseMerkleTree<Account, Fr, PedersenHasher<Engine>>;

pub type TransactionSignature = tx::TransactionSignature<Engine>;
pub type Tx = tx::Tx<Engine>;
pub type TxBlock = block::Block<Tx, Engine>;

pub type DepositRequest = deposit::DepositRequest<Engine>;
pub type DepositBlock = block::Block<DepositRequest, Engine>;

pub type ExitRequest = exit::ExitRequest<Engine>;
pub type ExitBlock = block::Block<ExitRequest, Engine>;

#[derive(Clone)]
pub enum Block {
    Tx(TxBlock),
    Deposit(DepositBlock),
    Exit(ExitBlock)
}

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

    pub fn apply(&mut self, transaction: &Tx) -> Result<(), ()> {

        let from_account = field_element_to_u32(transaction.from);
        let mut from = self.balance_tree.items.get(&from_account).ok_or(())?.clone();

        // TODO: compare balances correctly!!!
        if field_element_to_u128(from.balance) < field_element_to_u128(transaction.amount) { return Err(()); }

        // TODO: check nonce: assert field_element_to_u32(from.nonce) == transaction.nonce

        // update state

        let to_account = field_element_to_u32(transaction.to);
        let mut to = self.balance_tree.items.get(&to_account).ok_or(())?.clone();
        let amount = Fr::from_str(&transaction.amount.to_string()).unwrap();
        from.balance.sub_assign(&amount);
        // TODO: subtract fee
        from.nonce.add_assign(&Fr::one());  // from.nonce++
        to.balance.add_assign(&amount);     // to.balance += amount
        self.balance_tree.insert(from_account, from);
        self.balance_tree.insert(to_account, to);

        Ok(())
    }

}