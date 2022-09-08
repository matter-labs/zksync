// External uses
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;
use zksync_types::withdrawals::WithdrawalPendingEvent;
// Local uses
use super::{
    types::{
        BlockInfo, BlockNumber, Bytes, CallRequest, Filter, Log, Transaction, TransactionReceipt,
        H160, H256, U256, U64,
    },
    Web3RpcApp,
};

pub type BoxFutureResult<T> = BoxFuture<Result<T>>;

macro_rules! spawn {
    ($self: ident.$method: ident($($args: expr),*)) => {{
        let self_ = $self.clone();
        Box::pin(self_.$method($($args),*))
    }}
}

#[rpc]
pub trait Web3Rpc {
    #[rpc(name = "net_version", returns = "String")]
    fn net_version(&self) -> Result<String>;

    #[rpc(name = "web3_clientVersion", returns = "String")]
    fn web3_client_version(&self) -> Result<String>;

    #[rpc(name = "eth_protocolVersion", returns = "String")]
    fn protocol_version(&self) -> Result<String>;

    #[rpc(name = "eth_mining", returns = "bool")]
    fn mining(&self) -> Result<bool>;

    #[rpc(name = "eth_hashrate", returns = "U256")]
    fn hashrate(&self) -> Result<U256>;

    #[rpc(name = "eth_gasPrice", returns = "U256")]
    fn gas_price(&self) -> Result<U256>;

    #[rpc(name = "eth_accounts", returns = "Vec<H160>")]
    fn accounts(&self) -> Result<Vec<H160>>;

    #[rpc(name = "eth_getUncleCountByBlockHash", returns = "U256")]
    fn get_uncle_count_by_block_hash(&self, block_hash: H256) -> Result<U256>;

    #[rpc(name = "eth_getUncleCountByBlockNumber", returns = "U256")]
    fn get_uncle_count_by_block_number(&self, block_number: BlockNumber) -> Result<U256>;

    #[rpc(name = "eth_blockNumber", returns = "U64")]
    fn block_number(&self) -> BoxFutureResult<U64>;

    #[rpc(name = "eth_getBalance", returns = "U256")]
    fn get_balance(&self, address: H160, block: Option<BlockNumber>) -> BoxFutureResult<U256>;

    #[rpc(name = "eth_getBlockTransactionCountByHash", returns = "Option<U256>")]
    fn get_block_transaction_count_by_hash(&self, hash: H256) -> BoxFutureResult<Option<U256>>;

    #[rpc(
        name = "eth_getBlockTransactionCountByNumber",
        returns = "Option<U256>"
    )]
    fn get_block_transaction_count_by_number(
        &self,
        block: Option<BlockNumber>,
    ) -> BoxFutureResult<Option<U256>>;

    #[rpc(name = "eth_getTransactionByHash", returns = "Option<Transaction>")]
    fn get_transaction_by_hash(&self, hash: H256) -> BoxFutureResult<Option<Transaction>>;

    #[rpc(name = "eth_getBlockByNumber", returns = "Option<BlockInfo>")]
    fn get_block_by_number(
        &self,
        block_number: Option<BlockNumber>,
        include_txs: bool,
    ) -> BoxFutureResult<Option<BlockInfo>>;

    #[rpc(name = "eth_getBlockByHash", returns = "Option<BlockInfo>")]
    fn get_block_by_hash(
        &self,
        hash: H256,
        include_txs: bool,
    ) -> BoxFutureResult<Option<BlockInfo>>;

    #[rpc(
        name = "eth_getTransactionReceipt",
        returns = "Option<TransactionReceipt>"
    )]
    fn get_transaction_receipt(&self, hash: H256) -> BoxFutureResult<Option<TransactionReceipt>>;

    #[rpc(name = "eth_getLogs", returns = "Vec<Log>")]
    fn get_logs(&self, filter: Filter) -> BoxFutureResult<Vec<Log>>;

    #[rpc(name = "eth_call", returns = "Bytes")]
    fn call(&self, req: CallRequest, _block: Option<BlockNumber>) -> BoxFutureResult<Bytes>;

    #[rpc(name = "zksync_checkWithdrawal", returns = "Vec<String>")]
    fn check_withdrawal(&self, tx_hash: H256) -> BoxFutureResult<Vec<WithdrawalPendingEvent>>;
}

impl Web3Rpc for Web3RpcApp {
    fn net_version(&self) -> Result<String> {
        Ok(self.chain_id.to_string())
    }

    fn web3_client_version(&self) -> Result<String> {
        Ok(String::from("zkSync"))
    }

    fn protocol_version(&self) -> Result<String> {
        Ok(String::from("0"))
    }

    fn mining(&self) -> Result<bool> {
        Ok(false)
    }

    fn hashrate(&self) -> Result<U256> {
        Ok(U256::zero())
    }

    fn gas_price(&self) -> Result<U256> {
        Ok(U256::zero())
    }

    fn accounts(&self) -> Result<Vec<H160>> {
        Ok(Vec::new())
    }

    fn get_uncle_count_by_block_hash(&self, _block_hash: H256) -> Result<U256> {
        Ok(U256::zero())
    }

    fn get_uncle_count_by_block_number(&self, _block_number: BlockNumber) -> Result<U256> {
        Ok(U256::zero())
    }

    fn block_number(&self) -> BoxFutureResult<U64> {
        spawn!(self._impl_block_number())
    }

    fn get_balance(&self, address: H160, block: Option<BlockNumber>) -> BoxFutureResult<U256> {
        spawn!(self._impl_get_balance(address, block))
    }

    fn get_block_transaction_count_by_hash(&self, hash: H256) -> BoxFutureResult<Option<U256>> {
        spawn!(self._impl_get_block_transaction_count_by_hash(hash))
    }

    fn get_block_transaction_count_by_number(
        &self,
        block: Option<BlockNumber>,
    ) -> BoxFutureResult<Option<U256>> {
        spawn!(self._impl_get_block_transaction_count_by_number(block))
    }

    fn get_transaction_by_hash(&self, hash: H256) -> BoxFutureResult<Option<Transaction>> {
        spawn!(self._impl_get_transaction_by_hash(hash))
    }

    fn get_block_by_number(
        &self,
        block_number: Option<BlockNumber>,
        include_txs: bool,
    ) -> BoxFutureResult<Option<BlockInfo>> {
        spawn!(self._impl_get_block_by_number(block_number, include_txs))
    }

    fn get_block_by_hash(
        &self,
        hash: H256,
        include_txs: bool,
    ) -> BoxFutureResult<Option<BlockInfo>> {
        spawn!(self._impl_get_block_by_hash(hash, include_txs))
    }

    fn get_transaction_receipt(&self, hash: H256) -> BoxFutureResult<Option<TransactionReceipt>> {
        spawn!(self._impl_get_transaction_receipt(hash))
    }

    fn get_logs(&self, filter: Filter) -> BoxFutureResult<Vec<Log>> {
        spawn!(self._impl_get_logs(filter))
    }

    fn call(&self, req: CallRequest, block: Option<BlockNumber>) -> BoxFutureResult<Bytes> {
        spawn! { self._impl_call(req, block) }
    }

    fn check_withdrawal(&self, tx_hash: H256) -> BoxFutureResult<Vec<WithdrawalPendingEvent>> {
        spawn! { self._impl_check_withdrawal(tx_hash) }
    }
}
