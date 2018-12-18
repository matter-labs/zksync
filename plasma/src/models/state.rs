use super::*;

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

    pub fn root_hash (&self) -> FieldBytes {
        self.balance_tree.root_hash().clone()
    }

    pub fn apply(&mut self, tx: &TransferTx) -> Result<(), ()> {

        let mut from = self.balance_tree.items.get(&tx.from).ok_or(())?.clone();

        // TODO: compare balances correctly!!!
        if from.balance < tx.amount { return Err(()); }
        if from.nonce != tx.nonce { return Err(()); }

        // update state

        let mut to = self.balance_tree.items.get(&tx.to).ok_or(())?.clone();
        from.balance -= tx.amount;
        
        // TODO: subtract fee

        from.nonce += 1;
        
        to.balance += tx.amount;

        self.balance_tree.insert(tx.from, from);
        self.balance_tree.insert(tx.to, to);

        Ok(())
    }

}