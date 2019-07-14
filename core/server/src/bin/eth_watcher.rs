use server::new_eth_watch::EthWatch;

fn main() {
    env_logger::init();

    let mut watcher = EthWatch::new();
    watcher.run();
}
