use server::eth_watch::EthWatch;
use storage::ConnectionPool;

fn main() {
    env_logger::init();
    let web3_url = std::env::var("WEB3_URL").expect("WEB3_URL env var not found");
    let governance_addr = std::env::var("GOVERNANCE_ADDR").expect("GOVERNANCE_ADDR env not found")
        [2..]
        .parse()
        .expect("Failed to parse GOVERNANCE_ADDR");
    let priority_queue_address = std::env::var("PRIORITY_QUEUE_ADDR")
        .expect("PRIORITY_QUEUE_ADDR env var not found")[2..]
        .parse()
        .expect("Failed to parse PRIORITY_QUEUE_ADDR");
    let (_eloop, transport) = web3::transports::Http::new(&web3_url).unwrap();
    let web3 = web3::Web3::new(transport);

    let watcher = EthWatch::new(
        web3,
        ConnectionPool::new(),
        governance_addr,
        priority_queue_address,
    );
    watcher.run();
}
