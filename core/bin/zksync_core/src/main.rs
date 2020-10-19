use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_core::{run_core, wait_for_tasks};
use zksync_storage::ConnectionPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    // handle ctrl+c
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("Ctrl+C signal send");
        })
        .expect("Error setting Ctrl+C handler");
    }
    let connection_pool = ConnectionPool::new(None).await;

    let task_handles = run_core(connection_pool, stop_signal_sender)
        .await
        .expect("Unable to start Core actors");

    tokio::select! {
        _ = async { wait_for_tasks(task_handles).await } => {
            // We don't need to do anything here, since actors will panic upon future resolving.
        },
        _ = async { stop_signal_receiver.next().await } => {
            log::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
