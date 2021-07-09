// Built-in uses
use std::time::Instant;
// External uses
use jsonrpc_core::{Error, Result};
// Local uses
use super::{
    types::{Address, BlockNumber, H256, U256, U64},
    Web3RpcApp,
};

impl Web3RpcApp {
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

    pub async fn _impl_block_number(self) -> Result<U64> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let block_number = storage
            .chain()
            .block_schema()
            .get_last_verified_confirmed_block()
            .await
            .map_err(|_| Error::internal_error())?;
        metrics::histogram!("api.web3.block_number", start.elapsed());
        Ok(U64::from(block_number.0))
    }

    pub async fn _impl_get_balance(
        self,
        address: zksync_types::Address,
        block: Option<BlockNumber>,
    ) -> Result<U256> {
        let start = Instant::now();
        let block_number = self
            .resolve_block_number(block)
            .await?
            .ok_or_else(|| Error::invalid_params("Block with such number doesn't exist yet"))?;
        let mut storage = self.access_storage().await?;
        let balance = storage
            .chain()
            .account_schema()
            .get_account_eth_balance_for_block(address, block_number)
            .await
            .map_err(|_| Error::internal_error())?;
        let result =
            U256::from_dec_str(&balance.to_string()).map_err(|_| Error::internal_error())?;
        metrics::histogram!("api.web3.get_balance", start.elapsed());
        Ok(result)
    }
}
