// Built-in uses
use std::sync::Arc;
// External uses
use actix::prelude::*;
use futures_util::stream::StreamExt;
// Workspace uses
use zksync_config::ZkSyncConfig;
use zksync_storage::{listener::StorageListener, ConnectionPool};
// Local uses
use crate::messages::{NewEvents, NewStorageEvent};
use crate::monitor::ServerMonitor;

pub struct EventListener {
    db_pool: ConnectionPool,
    server_monitor: Addr<ServerMonitor>,
    listener: Option<StorageListener>,
    last_processed_event_id: i64,
}

impl StreamHandler<NewStorageEvent> for EventListener {
    fn handle(&mut self, new_event: NewStorageEvent, ctx: &mut Self::Context) {
        if self.last_processed_event_id >= new_event.0 {
            return;
        }
        let pool = self.db_pool.clone();
        async move {
            pool.access_storage()
                .await
                .unwrap()
                .event_schema()
                .get_unprocessed_events()
                .await
                .unwrap()
        }
        .into_actor(self)
        .then(|events, act, _ctx| {
            if let Some(event) = events.last() {
                act.last_processed_event_id = event.id;
            }
            let msg = NewEvents(Arc::new(events));
            act.server_monitor.send(msg).into_actor(act)
        })
        .map(|response, _, _| {
            if let Err(err) = response {
                vlog::warn!(
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
        let stream = self
            .listener
            .take()
            .unwrap()
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
        let last_processed_event_id = db_pool
            .access_storage()
            .await?
            .event_schema()
            .get_last_processed_event_id()
            .await?
            .unwrap_or(0);

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
