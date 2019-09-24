use server::eth_watch::EthWatch;
use storage::ConnectionPool;

fn main() {
    env_logger::init();
    let web3_url = std::env::var("WEB3_URL").expect("WEB3_URL env var not found");
    let (_eloop, transport) = web3::transports::Http::new(&web3_url).unwrap();
    let web3 = web3::Web3::new(transport);

    let watcher = EthWatch::new(web3, ConnectionPool::new());
    watcher.run();
}
