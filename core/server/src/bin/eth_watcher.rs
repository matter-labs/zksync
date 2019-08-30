use server::eth_watch::EthWatch;

fn main() {
    env_logger::init();

    let watcher = EthWatch::new();
    watcher.run();
}
