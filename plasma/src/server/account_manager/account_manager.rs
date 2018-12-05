use std::sync::Arc;
use std::sync::mpsc::{channel, Sender, Receiver};

use super::rest_api::APIServer;

// Account manager API methods
#[derive(Serialize, Deserialize, Debug)]
pub enum APICall {

    Register{
        username: String,
        password: String,
    },

    SendTransaction{
        uid:    u32,
        tx:     String,
    }

}

pub trait APIHandler: Send + Sync {
    fn handle(&self, call: APICall);
}

/// Interface for interaction with users
pub struct AccountManager {

}

impl APIHandler for AccountManager {

    fn handle(&self, call: APICall) {
        println!("called {:?}", &call);
    }

}

impl AccountManager {

    pub fn new() -> Arc<Self> {
        let this = Arc::new(Self{});
        APIServer::new(&this);
        this
    }
}

