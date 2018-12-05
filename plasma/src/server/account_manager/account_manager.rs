use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};

use super::rest_api::APIServer;

// Account manager API methods
#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum APICall {

    Phony,

    Register{
        username: String,
        password: String,
    },

    SendTransaction{
        uid:    u32,
        tx:     String,
    }

}

/// Interface for interaction with users
pub struct AccountManager {

}

impl AccountManager {

    pub fn new() -> Arc<Self> {
        let this = Arc::new(Self{});
        APIServer::new(&this);
        this
    }

    pub fn handle(&self, call: APICall) {
        println!("called {:?}", &call);
    }
}


#[test]
fn test_account_manager() {
    let man = AccountManager::new();
    
    
}