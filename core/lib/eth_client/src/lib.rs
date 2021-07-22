pub mod clients;
pub mod ethereum_gateway;
pub use clients::http_client::ETHDirectClient;
pub use clients::multiplexer::MultiplexerEthereumClient;
pub use ethereum_gateway::{EthereumGateway, SignedCallResult};
