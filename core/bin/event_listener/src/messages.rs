use actix::prelude::*;
use std::sync::Arc;
use zksync_storage::{event::types::ZkSyncEvent, listener::notification::StorageNotification};

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
pub struct NewStorageEvent(pub i64);

impl From<StorageNotification> for NewStorageEvent {
    fn from(notification: StorageNotification) -> Self {
        Self(notification.payload().parse().unwrap())
    }
}
