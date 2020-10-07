//! Storage for subscription objects.
use super::SubscriptionSender;
use futures::{
    channel::{mpsc, oneshot},
    compat::Future01CompatExt,
    select,
    stream::StreamExt,
    FutureExt, SinkExt,
};
use std::collections::BTreeMap;
use zksync_types::ActionType;

use jsonrpc_pubsub::{
    typed::{Sink, Subscriber},
    SubscriptionId,
};

const MAX_LISTENERS_PER_ENTITY: usize = 2048;

pub struct SubStorage<ActionId, Response> {
    storage: BTreeMap<(ActionId, ActionType), Vec<SubscriptionSender<Response>>>,
}

impl<ActionId: std::cmp::Ord + Clone, Response> SubStorage<ActionId, Response> {
    pub fn new() -> Self {
        Self {
            storage: BTreeMap::default(),
        }
    }

    fn send_once<T: serde::Serialize>(&self, sink: &Sink<T>, val: T) {
        tokio::spawn(sink.notify(Ok(val)).compat().map(drop));
    }

    pub fn insert_new(
        &mut self,
        sub_id: SubscriptionId,
        sub: Subscriber<Response>,
        action_id: ActionId,
        action_type: ActionType,
    ) -> anyhow::Result<()> {
        let mut subs = self
            .storage
            .remove(&(action_id.clone(), action_type))
            .unwrap_or_default();
        if subs.len() < MAX_LISTENERS_PER_ENTITY {
            let sink = sub
                .assign_id(sub_id.clone())
                .map_err(|_| anyhow::format_err!("SubIdAssign"))?;
            subs.push(SubscriptionSender { id: sub_id, sink });
        };
        self.storage.insert((action_id, action_type), subs);

        Ok(())
    }
}
