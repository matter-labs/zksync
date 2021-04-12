use futures::channel::mpsc::channel;
use zksync_event_listener::handler::run_event_handler;
use zksync_event_listener::listener::run_event_listener;
// use zksync_config::ZkSyncConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    const MAX_CHANNEL_SIZE: usize = 32768;
    let (sender, receiver) = channel(MAX_CHANNEL_SIZE);
    let event_listener_task = run_event_listener(sender);
    let event_handler_task = run_event_handler(receiver);

    tokio::select! {
        _ = async { event_listener_task.await } => {
            vlog::warn!("Event listener unexpectedly terminated");
        },
        _ = async { event_handler_task.await } => {
            vlog::warn!("Event handler unexpectedly terminated");
        },
    }

    Ok(())
}
