use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_config::ConfigurationOptions;
use zksync_eth_sender::run_eth_sender;
use zksync_storage::ConnectionPool;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `eth_sender` doesn't require many connections to the database.
    const ETH_SENDER_CONNECTION_POOL_SIZE: u32 = 2;

    env_logger::init();

    // handle ctrl+c
    let (stop_signal_sender, mut stop_signal_receiver) = mpsc::channel(256);
    {
        let stop_signal_sender = RefCell::new(stop_signal_sender.clone());
        ctrlc::set_handler(move || {
            let mut sender = stop_signal_sender.borrow_mut();
            block_on(sender.send(true)).expect("crtlc signal send");
        })
        .expect("Error setting Ctrl-C handler");
    }

    let pool = ConnectionPool::new(Some(ETH_SENDER_CONNECTION_POOL_SIZE)).await;
    let config_options = ConfigurationOptions::from_env();

    let task_handle = run_eth_sender(pool, config_options);

    tokio::select! {
        _ = async { task_handle.await } => {
            panic!("Ethereum sender actors aren't supposed to finish their execution")
        },
        _ = async { stop_signal_receiver.next().await } => {
            log::warn!("Stop signal received, shutting down");
        }
    };

    Ok(())
}
