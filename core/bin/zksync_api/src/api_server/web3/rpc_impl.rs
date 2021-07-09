// Built-in uses
use std::time::Instant;
// External uses
use jsonrpc_core::Result;
// Local uses
use super::{
    types::{Address, BlockNumber, H256, U256},
    Web3RpcApp,
};

impl Web3RpcApp {
    pub async fn _impl_ping(self) -> Result<bool> {
        let start = Instant::now();
        metrics::histogram!("api.web3.ping", start.elapsed());
        Ok(true)
    }

    pub async fn _impl_web3_client_version(self) -> Result<String> {
        Ok(String::from("ZkSync"))
    }

    pub async fn _impl_net_version(self) -> Result<String> {
        Ok(self.chain_id.to_string())
    }

    pub async fn _impl_protocol_version(self) -> Result<String> {
        Ok(String::from("0"))
    }

    pub async fn _impl_mining(self) -> Result<bool> {
        Ok(false)
    }

    pub async fn _impl_hashrate(self) -> Result<U256> {
        Ok(U256::zero())
    }

    pub async fn _impl_gas_price(self) -> Result<U256> {
        Ok(U256::zero())
    }

    pub async fn _impl_accounts(self) -> Result<Vec<Address>> {
        Ok(Vec::new())
    }

    pub async fn _impl_get_uncle_count_by_block_hash(self, _block_hash: H256) -> Result<U256> {
        Ok(U256::zero())
    }

    pub async fn _impl_get_uncle_count_by_block_number(
        self,
        _block_number: BlockNumber,
    ) -> Result<U256> {
        Ok(U256::zero())
    }
}
