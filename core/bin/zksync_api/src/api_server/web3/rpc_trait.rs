// External uses
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;
// Local uses
use super::{
    types::{Address, BlockNumber, H256, U256},
    Web3RpcApp,
};

pub type FutureResp<T> = Box<dyn futures01::Future<Item = T, Error = Error> + Send>;

macro_rules! spawn {
    ($self: ident$(.$method: ident($($args: expr),*))+) => {{
        let handle = $self.runtime_handle.clone();
        let self_ = $self.clone();
        let resp = async move {
            handle
                .spawn(self_$(.$method($($args),*))+)
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }}
}

#[rpc]
pub trait Web3Rpc {
    #[rpc(name = "ping", returns = "bool")]
    fn ping(&self) -> FutureResp<bool>;

    #[rpc(name = "web3_clientVersion", returns = "String")]
    fn web3_client_version(&self) -> Result<String, Error>;

    #[rpc(name = "net_version", returns = "String")]
    fn net_version(&self) -> Result<String, Error>;

    #[rpc(name = "eth_protocolVersion", returns = "String")]
    fn protocol_version(&self) -> Result<String, Error>;

    #[rpc(name = "eth_mining", returns = "bool")]
    fn mining(&self) -> Result<bool, Error>;

    #[rpc(name = "eth_hashrate", returns = "U256")]
    fn hashrate(&self) -> Result<U256, Error>;

    #[rpc(name = "eth_gasPrice", returns = "U256")]
    fn gas_price(&self) -> Result<U256, Error>;

    #[rpc(name = "eth_accounts", returns = "Vec<Address>")]
    fn accounts(&self) -> Result<Vec<Address>, Error>;

    #[rpc(name = "eth_getUncleCountByBlockHash", returns = "U256")]
    fn get_uncle_count_by_block_hash(&self, block_hash: H256) -> Result<U256, Error>;

    #[rpc(name = "eth_getUncleCountByBlockNumber", returns = "U256")]
    fn get_uncle_count_by_block_number(&self, block_number: BlockNumber) -> Result<U256, Error>;
}

impl Web3Rpc for Web3RpcApp {
    fn ping(&self) -> FutureResp<bool> {
        spawn! { self._impl_ping() }
    }

    fn web3_client_version(&self) -> Result<String, Error> {
        Ok(String::from("zkSync"))
    }

    fn net_version(&self) -> Result<String, Error> {
        Ok(self.chain_id.to_string())
    }

    fn protocol_version(&self) -> Result<String, Error> {
        Ok(String::from("0"))
    }

    fn mining(&self) -> Result<bool, Error> {
        Ok(false)
    }

    fn hashrate(&self) -> Result<U256, Error> {
        Ok(U256::zero())
    }

    fn gas_price(&self) -> Result<U256, Error> {
        Ok(U256::zero())
    }

    fn accounts(&self) -> Result<Vec<Address>, Error> {
        Ok(Vec::new())
    }

    fn get_uncle_count_by_block_hash(&self, _block_hash: H256) -> Result<U256, Error> {
        Ok(U256::zero())
    }

    fn get_uncle_count_by_block_number(&self, _block_number: BlockNumber) -> Result<U256, Error> {
        Ok(U256::zero())
    }
}
