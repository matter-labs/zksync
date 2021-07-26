// Built-in uses
use std::{convert::TryFrom, fmt::Display, sync::Arc};
// External uses
use actix::prelude::*;
use futures_util::{future::Either, stream::StreamExt};
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{listener::StorageListener, ConnectionPool};
use zksync_types::event::{EventId, ZkSyncEvent};
// Local uses
use crate::messages::{NewEvents, NewStorageEvent, Shutdown};
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

type NotifyResult = anyhow::Result<NewStorageEvent>;

impl StreamHandler<NotifyResult> for EventListener {
    fn handle(&mut self, new_event: NotifyResult, ctx: &mut Self::Context) {
        // If we encounter an error during event processing, the actor
        // sends a shutdown message to the monitor and stops its context.
        let new_event = match new_event {
            Ok(event) => event,
            Err(err) => return self.shutdown(err).wait(ctx),
        };
        // The listener gets notified about every new row in the `events`
        // table, however we fetch them in packs. If new event's id is less
        // than our tracked offset, skip the message processing.
        if self.last_processed_event_id >= new_event.0 {
            return;
        }
        // - Try to fetch latest events from the database.
        // - If any of the storage methods returned an error, or we couldn't
        // deserialize new events, send `Shutdown` message to the monitor.
        // - Otherwise, wrap new events into `Arc`, send them to the monitor
        // and update the offset.
        // - Depending on the outcome of the second step, either log an error
        // or stop the actor's context.
        let pool = self.db_pool.clone();
        let last_processed_event_id = self.last_processed_event_id;
        async move {
            // Try to fetch and deserialize new events.
            Ok(pool
                .access_storage()
                .await?
                .event_schema()
                .fetch_new_events(last_processed_event_id)
                .await?
                .into_iter()
                .map(ZkSyncEvent::try_from)
                .collect::<Result<_, _>>()?)
        }
        .into_actor(self)
        .then(|result: anyhow::Result<Vec<ZkSyncEvent>>, act, _| {
            let mut shutdown = false;
            match result {
                Ok(events) => {
                    // Update the offset.
                    if let Some(event) = events.last() {
                        act.last_processed_event_id = event.id;
                    }
                    // We don't process new notifications until we send the message.
                    let msg = NewEvents(Arc::new(events));
                    Either::Left(act.server_monitor.send(msg))
                }
                Err(err) => {
                    // A database error ocurred, stop the actor's context.
                    vlog::error!(
                        "An error ocurred: {}, shutting down the EventListener actor",
                        err.to_string()
                    );
                    shutdown = true;
                    Either::Right(act.server_monitor.send(Shutdown))
                }
            }
            .into_actor(act)
            .map(move |response, _, ctx| {
                if let Err(err) = response {
                    vlog::error!(
                        "Couldn't send new events to server monitor, reason: {:?}",
                        err
                    );
                }
                if shutdown {
                    ctx.stop();
                }
            })
        })
        .wait(ctx);
    }

    fn finished(&mut self, ctx: &mut Self::Context) {
        self.shutdown("notifications stream is finished").wait(ctx);
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
            .map(|item| item.and_then(NewStorageEvent::try_from));
        Self::add_stream(stream, ctx);
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        vlog::warn!("EventListener actor has stopped");
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

    /// Returns the future that can be spawned on the actor's context
    /// in order to initiate the shutdown of the event server.
    ///
    /// # Arguments
    ///
    /// * `err` - human-readable reason for the shutdown
    ///
    fn shutdown<E: Display>(&mut self, err: E) -> impl ContextFutureSpawner<Self> {
        vlog::error!(
            "An error ocurred: {}, shutting down the EventListener actor",
            err.to_string()
        );
        self.server_monitor
            .send(Shutdown)
            .into_actor(self)
            .map(|_, _, ctx| ctx.stop())
    }
}
