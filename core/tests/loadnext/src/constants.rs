use std::time::Duration;

/// Normally, block is committed on Ethereum every 15 seconds; however there are no guarantees that our transaction
/// will be included in the next block right after sending.
pub const ETH_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(300);
/// Loadtest assumes that blocks on the server will be created relatively quickly (without timeouts set in hours),
/// but nonetheless we want to provide some buffer in case we'll spam the server with way too many transactions
/// and some tx will have to wait in the mempool for a while.
pub const COMMIT_TIMEOUT: Duration = Duration::from_secs(600);
/// We don't want to overload the server with too many requests; given the fact that blocks are expected to be created
/// every couple of seconds, chosen value seems to be adequate to provide the result in one or two calls at average.
pub const POLLING_INTERVAL: Duration = Duration::from_secs(3);

// TODO (ZKS-623): This value is not the greatest batch size zkSync supports.
// However, choosing the bigger value (e.g. 40) causes server to fail with error "Error communicating core server".
pub const MAX_BATCH_SIZE: usize = 20;
