use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};

use super::rest_api::{APIServer};

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

pub struct AccountHandler {

}

impl AccountHandler {
    pub fn handle(&self, call: APICall) {
        println!("called {:?}", &call);
    }
}


/// Interface for interaction with users
pub struct AccountManager {
    //handler: Arc<AccountHandler>,
}

impl AccountManager {

    pub fn new() -> Self {
        let this = Self{};
        let handler = Arc::new(AccountHandler{});
        APIServer::new(Arc::clone(&handler));
        this
    }
}
