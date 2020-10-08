use std::collections::HashMap;
// External uses
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;
// Workspace uses
use zksync_types::{
    tx::{TxEthSignature, TxHash},
    Address, Token, TokenLike, TxFeeTypes, ZkSyncTx,
};

// Local uses
use crate::fee_ticker::{BatchFee, Fee};
use bigdecimal::BigDecimal;

use super::{types::*, RpcApp};

pub type FutureResp<T> = Box<dyn futures01::Future<Item = T, Error = Error> + Send>;

#[rpc]
pub trait Rpc {
    #[rpc(name = "account_info", returns = "AccountInfoResp")]
    fn account_info(&self, addr: Address) -> FutureResp<AccountInfoResp>;

    #[rpc(name = "ethop_info", returns = "ETHOpInfoResp")]
    fn ethop_info(&self, serial_id: u32) -> FutureResp<ETHOpInfoResp>;

    #[rpc(name = "tx_info", returns = "ETHOpInfoResp")]
    fn tx_info(&self, hash: TxHash) -> FutureResp<TransactionInfoResp>;

    #[rpc(name = "tx_submit", returns = "TxHash")]
    fn tx_submit(
        &self,
        tx: Box<ZkSyncTx>,
        signature: Box<Option<TxEthSignature>>,
        fast_processing: Option<bool>,
    ) -> FutureResp<TxHash>;

    #[rpc(name = "submit_txs_batch", returns = "Vec<TxHash>")]
    fn submit_txs_batch(&self, txs: Vec<TxWithSignature>) -> FutureResp<Vec<TxHash>>;

    #[rpc(name = "contract_address", returns = "ContractAddressResp")]
    fn contract_address(&self) -> FutureResp<ContractAddressResp>;

    /// "ETH" | #ERC20_ADDRESS => {Token}
    #[rpc(name = "tokens", returns = "Token")]
    fn tokens(&self) -> FutureResp<HashMap<String, Token>>;

    #[rpc(name = "get_tx_fee", returns = "Fee")]
    fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token_like: TokenLike,
    ) -> FutureResp<Fee>;

    #[rpc(name = "get_txs_batch_fee_in_wei", returns = "BatchFee")]
    fn get_txs_batch_fee_in_wei(
        &self,
        tx_types: Vec<TxFeeTypes>,
        addresses: Vec<Address>,
        token_like: TokenLike,
    ) -> FutureResp<BatchFee>;

    #[rpc(name = "get_token_price", returns = "BigDecimal")]
    fn get_token_price(&self, token_like: TokenLike) -> FutureResp<BigDecimal>;

    #[rpc(name = "get_confirmations_for_eth_op_amount", returns = "u64")]
    fn get_confirmations_for_eth_op_amount(&self) -> FutureResp<u64>;

    #[rpc(name = "get_eth_tx_for_withdrawal", returns = "Option<String>")]
    fn get_eth_tx_for_withdrawal(&self, withdrawal_hash: TxHash) -> FutureResp<Option<String>>;
}

impl Rpc for RpcApp {
    fn account_info(&self, addr: Address) -> FutureResp<AccountInfoResp> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move { handle.spawn(self_._impl_account_info(addr)).await.unwrap() };
        Box::new(resp.boxed().compat())
    }

    fn ethop_info(&self, serial_id: u32) -> FutureResp<ETHOpInfoResp> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_ethop_info(serial_id))
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }

    fn tx_info(&self, hash: TxHash) -> FutureResp<TransactionInfoResp> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move { handle.spawn(self_._impl_tx_info(hash)).await.unwrap() };
        Box::new(resp.boxed().compat())
    }

    fn tx_submit(
        &self,
        tx: Box<ZkSyncTx>,
        signature: Box<Option<TxEthSignature>>,
        fast_processing: Option<bool>,
    ) -> FutureResp<TxHash> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_tx_submit(tx, signature, fast_processing))
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }

    fn submit_txs_batch(&self, txs: Vec<TxWithSignature>) -> FutureResp<Vec<TxHash>> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_submit_txs_batch(txs))
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }

    fn contract_address(&self) -> FutureResp<ContractAddressResp> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move { handle.spawn(self_._impl_contract_address()).await.unwrap() };
        Box::new(resp.boxed().compat())
    }

    fn tokens(&self) -> FutureResp<HashMap<String, Token>> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move { handle.spawn(self_._impl_tokens()).await.unwrap() };
        Box::new(resp.boxed().compat())
    }

    fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token_like: TokenLike,
    ) -> FutureResp<Fee> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_get_tx_fee(tx_type, address, token_like))
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }

    fn get_txs_batch_fee_in_wei(
        &self,
        tx_types: Vec<TxFeeTypes>,
        addresses: Vec<Address>,
        token_like: TokenLike,
    ) -> FutureResp<BatchFee> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_get_txs_batch_fee_in_wei(tx_types, addresses, token_like))
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }

    fn get_token_price(&self, token_like: TokenLike) -> FutureResp<BigDecimal> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_get_token_price(token_like))
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }

    fn get_confirmations_for_eth_op_amount(&self) -> FutureResp<u64> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_get_confirmations_for_eth_op_amount())
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }

    fn get_eth_tx_for_withdrawal(&self, withdrawal_hash: TxHash) -> FutureResp<Option<String>> {
        let handle = self.runtime_handle.clone();
        let self_ = self.clone();
        let resp = async move {
            handle
                .spawn(self_._impl_get_eth_tx_for_withdrawal(withdrawal_hash))
                .await
                .unwrap()
        };
        Box::new(resp.boxed().compat())
    }
}
