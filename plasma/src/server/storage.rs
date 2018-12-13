use crate::models::plasma_models::{Block, PlasmaState}

struct StateStorage {

}

impl StateStorage {

    /// creates connection pool
    pub fn new() -> Self {
        Self{}
    }

    /// returns promise
    pub fn commit_block(block &Block) {

    }

    /// returns promise
    pub fn update_state(state &PlasmaState) {

    }

    /// returns stream of accounts
    pub fn load_state() {

    }

}

#[test]
fn storage_test() {

}