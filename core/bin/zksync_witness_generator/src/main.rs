use futures::{channel::mpsc, executor::block_on, SinkExt, StreamExt};
use std::cell::RefCell;
use zksync_config::{ConfigurationOptions, ProverOptions};
use zksync_storage::ConnectionPool;
use zksync_witness_generator::run_prover_server;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `eth_sender` doesn't require many connections to the database.
    const WITNESS_GENERATOR_CONNECTION_POOL_SIZE: u32 = 2;

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

    let connection_pool = ConnectionPool::new(Some(WITNESS_GENERATOR_CONNECTION_POOL_SIZE)).await;
    let config_options = ConfigurationOptions::from_env();
    let prover_options = ProverOptions::from_env();

    run_prover_server(
        connection_pool,
        stop_signal_sender,
        prover_options,
        config_options,
    );

    stop_signal_receiver.next().await;

    Ok(())
}
