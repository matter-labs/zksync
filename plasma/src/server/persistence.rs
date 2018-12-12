use crate::models::block::Block;
use crate::models::tx::TxUnpacked;

struct PersistenceManager {

}

impl PersistenceManager {

    /// creates connection pool
    pub fn new() -> Self {
        Self{}
    }

    /// returns promise
    pub fn commit_block(block &Block, accounts: Index<&u32, Output=Account>) {
        // insert block
        // update accounts
    }

    /// returns stream of accounts
    pub fn load_accounts() {

    }

}

#[test]
fn persistence_test() {

}