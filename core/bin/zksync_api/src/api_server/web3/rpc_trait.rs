// External uses
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;
// Local uses
use super::{
    types::{Address, BlockInfo, BlockNumber, Transaction, H256, U256, U64},
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
    #[rpc(name = "web3_clientVersion", returns = "String")]
    fn web3_client_version(&self) -> FutureResp<String>;

    #[rpc(name = "net_version", returns = "String")]
    fn net_version(&self) -> FutureResp<String>;

    #[rpc(name = "eth_protocolVersion", returns = "String")]
    fn protocol_version(&self) -> FutureResp<String>;

    #[rpc(name = "eth_mining", returns = "bool")]
    fn mining(&self) -> FutureResp<bool>;

    #[rpc(name = "eth_hashrate", returns = "U256")]
    fn hashrate(&self) -> FutureResp<U256>;

    #[rpc(name = "eth_gasPrice", returns = "U256")]
    fn gas_price(&self) -> FutureResp<U256>;

    #[rpc(name = "eth_accounts", returns = "Vec<Address>")]
    fn accounts(&self) -> FutureResp<Vec<Address>>;

    #[rpc(name = "eth_getUncleCountByBlockHash", returns = "U256")]
    fn get_uncle_count_by_block_hash(&self, block_hash: H256) -> FutureResp<U256>;

    #[rpc(name = "eth_getUncleCountByBlockNumber", returns = "U256")]
    fn get_uncle_count_by_block_number(&self, block_number: BlockNumber) -> FutureResp<U256>;

    #[rpc(name = "eth_blockNumber", returns = "U64")]
    fn block_number(&self) -> FutureResp<U64>;

    #[rpc(name = "eth_getBalance", returns = "U256")]
    fn get_balance(
        &self,
        address: zksync_types::Address,
        block: Option<BlockNumber>,
    ) -> FutureResp<U256>;

    #[rpc(name = "eth_getBlockTransactionCountByHash", returns = "Option<U256>")]
    fn get_block_transaction_count_by_hash(&self, hash: H256) -> FutureResp<Option<U256>>;

    #[rpc(
        name = "eth_getBlockTransactionCountByNumber",
        returns = "Option<U256>"
    )]
    fn get_block_transaction_count_by_number(
        &self,
        block: Option<BlockNumber>,
    ) -> FutureResp<Option<U256>>;

    #[rpc(name = "eth_getTransactionByHash", returns = "Option<Transaction>")]
    fn get_transaction_by_hash(&self, hash: H256) -> FutureResp<Option<Transaction>>;

    #[rpc(name = "eth_getBlockByNumber", returns = "Option<BlockInfo>")]
    fn get_block_by_number(
        &self,
        block_number: Option<BlockNumber>,
        include_txs: bool,
    ) -> FutureResp<Option<BlockInfo>>;

    #[rpc(name = "eth_getBlockByHash", returns = "Option<BlockInfo>")]
    fn get_block_by_hash(&self, hash: H256, include_txs: bool) -> FutureResp<Option<BlockInfo>>;
}

impl Web3Rpc for Web3RpcApp {
    fn web3_client_version(&self) -> FutureResp<String> {
        spawn! { self._impl_web3_client_version() }
    }

    fn net_version(&self) -> FutureResp<String> {
        spawn! { self._impl_net_version() }
    }

    fn protocol_version(&self) -> FutureResp<String> {
        spawn! { self._impl_protocol_version() }
    }

    fn mining(&self) -> FutureResp<bool> {
        spawn! { self._impl_mining() }
    }

    fn hashrate(&self) -> FutureResp<U256> {
        spawn! { self._impl_hashrate() }
    }

    fn gas_price(&self) -> FutureResp<U256> {
        spawn! { self._impl_gas_price() }
    }

    fn accounts(&self) -> FutureResp<Vec<Address>> {
        spawn! { self._impl_accounts() }
    }

    fn get_uncle_count_by_block_hash(&self, block_hash: H256) -> FutureResp<U256> {
        spawn! { self._impl_get_uncle_count_by_block_hash(block_hash) }
    }

    fn get_uncle_count_by_block_number(&self, block_number: BlockNumber) -> FutureResp<U256> {
        spawn! { self._impl_get_uncle_count_by_block_number(block_number) }
    }

    fn block_number(&self) -> FutureResp<U64> {
        spawn! { self._impl_block_number() }
    }

    fn get_balance(
        &self,
        address: zksync_types::Address,
        block: Option<BlockNumber>,
    ) -> FutureResp<U256> {
        spawn! { self._impl_get_balance(address, block) }
    }

    fn get_block_transaction_count_by_hash(&self, hash: H256) -> FutureResp<Option<U256>> {
        spawn! { self._impl_get_block_transaction_count_by_hash(hash) }
    }

    fn get_block_transaction_count_by_number(
        &self,
        block: Option<BlockNumber>,
    ) -> FutureResp<Option<U256>> {
        spawn! { self._impl_get_block_transaction_count_by_number(block) }
    }

    fn get_transaction_by_hash(&self, hash: H256) -> FutureResp<Option<Transaction>> {
        spawn! { self._impl_get_transaction_by_hash(hash) }
    }

    fn get_block_by_number(
        &self,
        block_number: Option<BlockNumber>,
        include_txs: bool,
    ) -> FutureResp<Option<BlockInfo>> {
        spawn! { self._impl_get_block_by_number(block_number, include_txs) }
    }

    fn get_block_by_hash(&self, hash: H256, include_txs: bool) -> FutureResp<Option<BlockInfo>> {
        spawn! { self._impl_get_block_by_hash(hash, include_txs) }
    }
}
