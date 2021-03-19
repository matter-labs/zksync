// use zksync_config::ZkSyncConfig;
use zksync_storage::{listener::StorageListener, StorageProcessor};

pub struct EventListener<'a> {
    _storage: StorageProcessor<'a>,
    listener: StorageListener,
    channel: String,
}

impl<'a> EventListener<'a> {
    // pub async fn new<'b>(config: ZkSyncConfig) -> anyhow::Result<EventListener<'b>> {
    pub async fn new<'b>() -> anyhow::Result<EventListener<'b>> {
        let _storage = StorageProcessor::establish_connection().await?;
        let listener = StorageListener::connect().await?;
        // let channel = config.db.listen_channel_name;
        let channel = "event_channel".into();
        Ok(EventListener {
            _storage,
            listener,
            channel,
        })
    }

    pub async fn run(&mut self) -> anyhow::Result<()> {
        self.listener.listen(&self.channel).await?;

        loop {
            while let Some(notification) = self.listener.try_recv().await? {
                println!(
                    "Received notification, payload:\n{}",
                    notification.payload()
                );
            }
            // Connection aborted, handle it here.
        }
    }
}
