//! Storage for subscription objects.
use super::SubscriptionSender;
use futures::{compat::Future01CompatExt, FutureExt};
use std::{cmp::Ord, collections::BTreeMap, str::FromStr};
use zksync_types::{tx::TxHash, AccountId, ActionType, PriorityOpId};

use jsonrpc_pubsub::{
    typed::{Sink, Subscriber},
    SubscriptionId,
};

const MAX_LISTENERS_PER_ENTITY: usize = 2048;
const TX_SUB_PREFIX: &str = "txsub";
const ETHOP_SUB_PREFIX: &str = "eosub";
const ACCOUNT_SUB_PREFIX: &str = "acsub";

pub trait ActionId {
    fn sub_type() -> &'static str;
}

impl ActionId for PriorityOpId {
    fn sub_type() -> &'static str {
        ETHOP_SUB_PREFIX
    }
}

impl ActionId for TxHash {
    fn sub_type() -> &'static str {
        TX_SUB_PREFIX
    }
}

impl ActionId for AccountId {
    fn sub_type() -> &'static str {
        ACCOUNT_SUB_PREFIX
    }
}

#[derive(Debug)]
pub struct SubStorage<ID, RESP> {
    storage: BTreeMap<(ID, ActionType), Vec<SubscriptionSender<RESP>>>,
}

impl<ID, RESP> SubStorage<ID, RESP>
where
    ID: Ord + Clone + ToString + FromStr + ActionId + std::fmt::Debug,
    RESP: serde::Serialize + Clone + std::fmt::Debug,
{
    pub fn new() -> Self {
        Self {
            storage: BTreeMap::default(),
        }
    }

    fn send_once(&self, sink: &Sink<RESP>, val: RESP) {
        tokio::spawn(sink.notify(Ok(val)).compat().map(drop));
    }

    pub fn generate_sub_id(&mut self, action_id: ID, action_type: ActionType) -> SubscriptionId {
        SubscriptionId::String(format!(
            "{}/{}/{}/{}",
            ID::sub_type(),
            action_id.to_string(),
            action_type.to_string(),
            zksync_crypto::rand::random::<u64>()
        ))
    }

    fn parse_sub_id(&self, sub_id: &str) -> anyhow::Result<Option<(ID, ActionType)>> {
        let incorrect_id_err = || anyhow::format_err!("Incorrect id: {:?}", sub_id);

        let mut id_split = sub_id.split('/').collect::<Vec<&str>>().into_iter();
        let sub_type = id_split.next().ok_or_else(incorrect_id_err)?;
        let sub_action_id = id_split.next().ok_or_else(incorrect_id_err)?;
        let sub_action_type = id_split.next().ok_or_else(incorrect_id_err)?;

        if sub_type != ID::sub_type() {
            // Not our type, do nothing.
            return Ok(None);
        }

        let action_id = sub_action_id.parse().map_err(|_| incorrect_id_err())?;
        let action_type = sub_action_type.parse().map_err(|_| incorrect_id_err())?;

        Ok(Some((action_id, action_type)))
    }

    pub fn insert_new(
        &mut self,
        sub_id: SubscriptionId,
        sub: Subscriber<RESP>,
        action_id: ID,
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

    pub fn remove(&mut self, sub_id: SubscriptionId) -> anyhow::Result<()> {
        let str_sub_id = if let SubscriptionId::String(str_sub_id) = sub_id.clone() {
            str_sub_id
        } else {
            anyhow::bail!("SubscriptionId should be String");
        };

        let (action_id, action_type) = match self.parse_sub_id(&str_sub_id)? {
            Some(id) => id,
            None => {
                return Ok(());
            }
        };

        if let Some(mut subs) = self.storage.remove(&(action_id.clone(), action_type)) {
            subs.retain(|sub| sub.id != sub_id);
            if !subs.is_empty() {
                self.storage.insert((action_id, action_type), subs);
            }
        }

        Ok(())
    }

    pub fn subscriber_exists(&mut self, action_id: ID, action_type: ActionType) -> bool {
        self.storage.contains_key(&(action_id, action_type))
    }

    pub fn notify(&mut self, action_id: ID, action_type: ActionType, event: RESP) {
        if let Some(subs) = self.storage.remove(&(action_id, action_type)) {
            for sub in subs {
                self.send_once(&sub.sink, event.clone());
            }
        }
    }

    pub fn respond_once(
        &mut self,
        sub_id: SubscriptionId,
        sub: Subscriber<RESP>,
        resp: RESP,
    ) -> anyhow::Result<()> {
        let sink = sub
            .assign_id(sub_id)
            .map_err(|_| anyhow::format_err!("SubIdAssign"))?;
        self.send_once(&sink, resp);

        Ok(())
    }
}
