// Built-in uses
use std::sync::Arc;
// External uses
use actix::prelude::*;
// Workspace uses
use zksync_storage::listener::notification::StorageNotification;
use zksync_types::event::{EventId, ZkSyncEvent};
// Local uses
use crate::subscriber::Subscriber;

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RegisterSubscriber(pub Addr<Subscriber>);

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RemoveSubscriber(pub Addr<Subscriber>);

#[derive(Debug, Clone, Message)]
#[rtype(result = "()")]
pub struct NewEvents(pub Arc<Vec<ZkSyncEvent>>);

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct NewStorageEvent(pub EventId);

impl From<StorageNotification> for NewStorageEvent {
    fn from(notification: StorageNotification) -> Self {
        Self(notification.payload().parse::<u64>().unwrap().into())
    }
}
