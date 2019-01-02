use super::*;
use crate::models::params;
use sapling_crypto::jubjub::{edwards, Unknown};

pub struct PlasmaState {

    /// Accounts stored in a sparse Merkle tree
    pub balance_tree: AccountTree,

    /// Current block number
    pub block_number: u32,
    
}

#[derive(Debug)]
pub enum TransferApplicationError {
    Unknown,
    InsufficientBalance,
    NonceIsTooLow,
    NonceIsTooHigh,
    InvalidSigner,
    InvalidTransaction(String),
}

impl PlasmaState {
    
    pub fn new(accounts: AccountMap, current_block: u32) -> Self {
        let tree_depth = params::BALANCE_TREE_DEPTH as u32;
        let mut balance_tree = AccountTree::new(tree_depth);
        for (id, account) in accounts {
            balance_tree.insert(id, account);
        }
        Self{
            balance_tree,
            block_number: current_block,
        }
    }

    pub fn get_accounts(&self) -> Vec<(u32, Account)> {
        self.balance_tree.items.iter().map(|a| (*a.0 as u32, a.1.clone()) ).collect()
    }

    pub fn get_pub_key(&self, account_id: u32) -> Option<PublicKey> {
        let item = self.balance_tree.items.get(&account_id);
        if item.is_none() {
            return None;
        }
        let acc = item.unwrap();
        let point = edwards::Point::<Engine, Unknown>::from_xy(
            acc.public_key_x, 
            acc.public_key_y, 
            &params::JUBJUB_PARAMS
        );
        if point.is_none() {
            return None;
        }

        let pk = sapling_crypto::eddsa::PublicKey::<Engine>(point.unwrap());

        Some(pk)
    }

    pub fn root_hash (&self) -> Fr {
        self.balance_tree.root_hash().clone()
    }

    pub fn apply_transfer(&mut self, tx: &TransferTx) -> Result<(), TransferApplicationError> {

        if let Some(mut from) = self.balance_tree.items.get(&tx.from).cloned() {
            // TODO: take from `from` instead and uncomment below
            let pub_key = self.get_pub_key(tx.from).unwrap();
            if let Some(verified_against) = tx.cached_pub_key.as_ref() {
                if pub_key.0 != verified_against.0 { 
                    return Err(TransferApplicationError::InvalidSigner);
                }   
            } else {
                return Err(TransferApplicationError::InvalidSigner);
            }
            
            if from.balance < tx.amount { 
                println!("Insufficient balance");
                return Err(TransferApplicationError::InsufficientBalance); 
            }
            if tx.nonce > from.nonce { 
                println!("Nonce is too high");
                return Err(TransferApplicationError::NonceIsTooHigh); 
            } else if tx.nonce < from.nonce {
                println!("Nonce is too low");
                return Err(TransferApplicationError::NonceIsTooLow); 
            }

            // update state

            // allow to send to non-existing accounts
            // let mut to = self.balance_tree.items.get(&tx.to).ok_or(())?.clone();
            
            let mut to = Account::default();
            if let Some(existing_to) = self.balance_tree.items.get(&tx.to) {
                to = existing_to.clone();
            }
            from.balance -= &tx.amount;
            
            // TODO: subtract fee

            from.nonce += 1;
            if tx.to != 0 {
                to.balance += &tx.amount;
            }

            self.balance_tree.insert(tx.from, from);
            self.balance_tree.insert(tx.to, to);

            return Ok(());
        }

        Err(TransferApplicationError::InvalidSigner)

       
    }

    pub fn apply_deposit(&mut self, tx: &DepositTx) -> Result<(), ()> {
        let existing_acc = self.balance_tree.items.get(&tx.account);

        if existing_acc.is_none() {
            let mut acc = Account::default();
            let tx = tx.clone();
            acc.public_key_x = tx.pub_x;
            acc.public_key_y = tx.pub_y;
            acc.balance = tx.amount;
            self.balance_tree.insert(tx.account, acc);
        } else {
            let mut acc = existing_acc.unwrap().clone();
            acc.balance += &tx.amount;
            self.balance_tree.insert(tx.account, acc);
        }
        Ok(())
    }

    pub fn apply_exit(&mut self, tx: &ExitTx) -> Result<ExitTx, ()> {

        let acc = self.balance_tree.items.get(&tx.account).ok_or(())?.clone();

        let mut agumented_tx = tx.clone();

        println!("Adding account balance to ExitTx, value = {}", acc.balance);

        agumented_tx.amount = acc.balance;

        self.balance_tree.delete(tx.account);

        Ok(agumented_tx)
    }

}