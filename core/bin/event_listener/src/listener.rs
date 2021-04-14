// TODO: Handle errors thoroughly once anyhow is removed from storage.
// use zksync_config::ZkSyncConfig;
use futures::channel::mpsc;
use futures::SinkExt;
use zksync_storage::{
    event::types::ZkSyncEvent,
    listener::{notification::StorageNotification, StorageListener},
    StorageProcessor,
};

pub struct EventListener<'a> {
    storage: StorageProcessor<'a>,
    listener: StorageListener,
    db_channel: String,
    last_processed_event_id: i64,
    events_sender: mpsc::Sender<Vec<ZkSyncEvent>>,
}

impl<'a> EventListener<'a> {
    // pub async fn new<'b>(config: ZkSyncConfig) -> anyhow::Result<EventListener<'b>> {
    pub async fn new<'b>(
        sender: mpsc::Sender<Vec<ZkSyncEvent>>,
    ) -> anyhow::Result<EventListener<'b>> {
        let listener = StorageListener::connect().await?;
        let mut storage = StorageProcessor::establish_connection().await?;
        let last_processed_event_id = storage
            .event_schema()
            .get_last_processed_event_id()
            .await?
            .unwrap_or(0);

        // let channel = config.db.listen_channel_name;
        let db_channel = "event_channel".into();
        Ok(EventListener {
            storage,
            listener,
            db_channel,
            last_processed_event_id,
            events_sender: sender,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.listener.listen(&self.db_channel).await?;

        // If the connection is aborted, it will be restored on the next
        // iteration, in this case we will try to fetch new events without
        // waiting for new notifications.
        // TODO: throttle processing.
        loop {
            match self.listener.try_recv().await? {
                Some(notification) => self.handle_notification(notification).await?,
                None => self.process_new_events().await?,
            }
        }
    }

    async fn handle_notification(
        &mut self,
        notification: StorageNotification,
    ) -> anyhow::Result<()> {
        let received_id: i64 = notification.payload().parse().unwrap();
        if self.last_processed_event_id >= received_id {
            return Ok(());
        }
        self.process_new_events().await?;
        Ok(())
    }

    async fn process_new_events(&mut self) -> anyhow::Result<()> {
        let events = self.storage.event_schema().get_unprocessed_events().await?;
        self.last_processed_event_id = match events.last() {
            Some(event) => event.id,
            None => return Ok(()),
        };
        // Send new events to the filtering component.
        let _ids: Vec<_> = events.iter().map(|event| event.id).collect();
        eprintln!("Fetched ids:\n{:?}", _ids);
        self.events_sender.send(events).await?;
        Ok(())
    }
}

#[must_use]
pub fn run_event_listener(sender: mpsc::Sender<Vec<ZkSyncEvent>>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async {
        let mut listener = EventListener::new(sender)
            .await
            .expect("couldn't intialize event listener");

        listener
            .run()
            .await
            .expect("an error happened while fetching new events from the database");
    })
}
