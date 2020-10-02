//! Additional methods gathering the information required
//! by ticker for operating.

// External deps
use async_trait::async_trait;
use futures::{
    channel::{mpsc, oneshot},
    SinkExt,
};
// Workspace deps
use zksync_types::{Account, AccountId, Address};
// Local deps
use crate::state_keeper::StateKeeperRequest;

/// Api responsible for querying for TokenPrices
#[async_trait]
pub trait FeeTickerInfo {
    /// Check whether account exists in the zkSync network or not.
    /// Returns `true` if account does not yet exist in the zkSync network.
    async fn is_account_new(&mut self, address: Address) -> bool;
}

pub struct TickerInfo {
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
}

impl TickerInfo {
    pub fn new(state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>) -> Self {
        Self {
            state_keeper_request_sender,
        }
    }
}

#[async_trait]
impl FeeTickerInfo for TickerInfo {
    async fn is_account_new(&mut self, address: Address) -> bool {
        let (account_info_sender, account_info_receiver) =
            oneshot::channel::<Option<(AccountId, Account)>>();

        self.state_keeper_request_sender
            .send(StateKeeperRequest::GetAccount(address, account_info_sender))
            .await
            .expect("State keeper receiver dropped");

        // If account is `Some(_)` then it's not new.
        account_info_receiver.await.unwrap().is_none()
    }
}
