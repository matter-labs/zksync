// Built-in uses
use std::{convert::TryFrom, sync::Arc};
// External uses
use actix::prelude::*;
use actix_web::dev::Server;
// Workspace uses
use zksync_storage::listener::notification::StorageNotification;
use zksync_types::event::{EventId, ZkSyncEvent};
// Local uses
use crate::subscriber::Subscriber;

/// Message emitted by the `EventListener` actor, indicates
/// that an internal error ocurred and the server should stop
/// accepting new connections as well as terminate existing ones.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct Shutdown;

/// This type of message is used to pass the ws-server
/// handle to the monitor on the system start. The handle
/// may be used to gracefully shutdown the server.
#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct RegisterServerHandle(pub Server);

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

impl TryFrom<StorageNotification> for NewStorageEvent {
    type Error = anyhow::Error;

    fn try_from(notification: StorageNotification) -> Result<Self, Self::Error> {
        Ok(Self(
            notification.payload().parse::<u64>().map(EventId::from)?,
        ))
    }
}
