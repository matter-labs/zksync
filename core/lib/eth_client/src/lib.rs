pub mod clients;
pub mod eth_client_trait;
pub use clients::http_client::ETHClient;
pub use clients::multiplexer::MultiPlexClient;
pub use eth_client_trait::SignedCallResult;
