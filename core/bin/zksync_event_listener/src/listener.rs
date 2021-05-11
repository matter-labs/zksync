// Built-in uses
use std::sync::Arc;
// External uses
use actix::prelude::*;
use futures_util::stream::StreamExt;
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{listener::StorageListener, ConnectionPool};
use zksync_types::event::EventId;
// Local uses
use crate::messages::{NewEvents, NewStorageEvent};
use crate::monitor::ServerMonitor;

/// The main actor which is responsible for fetching new events from
/// the database and sending them to the [`ServerMonitor`].
pub struct EventListener {
    /// Pool of connections to the database.
    db_pool: ConnectionPool,
    /// Address of the [`ServerMonitor`] actor for communication.
    server_monitor: Addr<ServerMonitor>,
    /// A storage listener that gets notified about new database events.
    /// This field gets consumed at the start of the actor.
    listener: Option<StorageListener>,
    /// The id of the last processed event.
    last_processed_event_id: EventId,
}

impl StreamHandler<NewStorageEvent> for EventListener {
    fn handle(&mut self, new_event: NewStorageEvent, ctx: &mut Self::Context) {
        // The listener gets notified about every new row in the `events`
        // table, however we fetch them in packs. If new event's id is less
        // than our tracked offset, skip the message processing.
        if self.last_processed_event_id >= new_event.0 {
            return;
        }
        let pool = self.db_pool.clone();
        let last_processed_event_id = self.last_processed_event_id;
        async move {
            pool.access_storage()
                .await
                .unwrap()
                .event_schema()
                .fetch_new_events(last_processed_event_id)
                .await
                .unwrap()
        }
        .into_actor(self)
        .then(|events, act, _ctx| {
            // Update the offset.
            if let Some(event) = events.last() {
                act.last_processed_event_id = event.id;
            }
            // We don't process new notifications until we send the message.
            let msg = NewEvents(Arc::new(events));
            act.server_monitor.send(msg).into_actor(act)
        })
        .map(|response, _, _| {
            if let Err(err) = response {
                vlog::error!(
                    "Couldn't send new events to server monitor, reason: {:?}",
                    err
                );
            }
        })
        .wait(ctx);
    }
}

impl Actor for EventListener {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        // Turn the storage listener into stream and register it.
        let stream = self
            .listener
            .take()
            .expect("storage listener is not initialized")
            .into_stream()
            .map(|item| NewStorageEvent::from(item.unwrap()));
        Self::add_stream(stream, ctx);
    }
}

impl EventListener {
    const DB_POOL_SIZE: u32 = 1;

    pub async fn new(
        server_monitor: Addr<ServerMonitor>,
        config: &ZkSyncConfig,
    ) -> anyhow::Result<EventListener> {
        let mut listener = StorageListener::connect().await?;
        let db_pool = ConnectionPool::new(Some(Self::DB_POOL_SIZE));
        // Load the offset, we don't want to broadcast events that already
        // happened.
        let last_processed_event_id = db_pool
            .access_storage()
            .await?
            .event_schema()
            .get_last_event_id()
            .await?
            .unwrap_or(EventId(0));

        // Configure the listener.
        let channel_name = &config.event_listener.channel_name;
        listener.listen(channel_name).await?;

        Ok(EventListener {
            db_pool,
            server_monitor,
            listener: Some(listener),
            last_processed_event_id,
        })
    }
}
