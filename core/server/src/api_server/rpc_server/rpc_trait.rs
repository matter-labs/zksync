use std::collections::HashMap;
// External uses
use futures::{FutureExt, TryFutureExt};
use jsonrpc_core::Error;
use jsonrpc_derive::rpc;
// Workspace uses
use models::node::{
    tx::{TxEthSignature, TxHash},
    Address, FranklinTx, Token, TokenLike, TxFeeTypes,
};
// use storage::{
//     chain::{
//         block::records::BlockDetails, operations::records::StoredExecutedPriorityOperation,
//         operations_ext::records::TxReceiptResponse,
//     },
//     ConnectionPool, StorageProcessor,
// };

// Local uses
use crate::fee_ticker::Fee;
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
        tx: Box<FranklinTx>,
        signature: Box<Option<TxEthSignature>>,
        fast_processing: Option<bool>,
    ) -> FutureResp<TxHash>;

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

    #[rpc(name = "get_token_price", returns = "BigDecimal")]
    fn get_token_price(&self, token_like: TokenLike) -> FutureResp<BigDecimal>;

    #[rpc(name = "get_confirmations_for_eth_op_amount", returns = "u64")]
    fn get_confirmations_for_eth_op_amount(&self) -> FutureResp<u64>;
}

impl Rpc for RpcApp {
    fn account_info(&self, addr: Address) -> FutureResp<AccountInfoResp> {
        let self_ = self.clone();
        let resp = self_._impl_account_info(addr);
        Box::new(resp.boxed().compat())
    }

    fn ethop_info(&self, serial_id: u32) -> FutureResp<ETHOpInfoResp> {
        let self_ = self.clone();
        let resp = self_._impl_ethop_info(serial_id);
        Box::new(resp.boxed().compat())
    }

    fn tx_info(&self, hash: TxHash) -> FutureResp<TransactionInfoResp> {
        let self_ = self.clone();
        let resp = self_._impl_tx_info(hash);
        Box::new(resp.boxed().compat())
    }

    fn tx_submit(
        &self,
        tx: Box<FranklinTx>,
        signature: Box<Option<TxEthSignature>>,
        fast_processing: Option<bool>,
    ) -> FutureResp<TxHash> {
        let self_ = self.clone();
        let resp = self_._impl_tx_submit(tx, signature, fast_processing);
        Box::new(resp.boxed().compat())
    }

    fn contract_address(&self) -> FutureResp<ContractAddressResp> {
        let self_ = self.clone();
        let resp = self_._impl_contract_address();
        Box::new(resp.boxed().compat())
    }

    fn tokens(&self) -> FutureResp<HashMap<String, Token>> {
        let self_ = self.clone();
        let resp = self_._impl_tokens();
        Box::new(resp.boxed().compat())
    }

    fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        address: Address,
        token_like: TokenLike,
    ) -> FutureResp<Fee> {
        let self_ = self.clone();
        let resp = self_._impl_get_tx_fee(tx_type, address, token_like);
        Box::new(resp.boxed().compat())
    }

    fn get_token_price(&self, token_like: TokenLike) -> FutureResp<BigDecimal> {
        let self_ = self.clone();
        let resp = self_._impl_get_token_price(token_like);
        Box::new(resp.boxed().compat())
    }

    fn get_confirmations_for_eth_op_amount(&self) -> FutureResp<u64> {
        let self_ = self.clone();
        let resp = self_._impl_get_confirmations_for_eth_op_amount();
        Box::new(resp.boxed().compat())
    }
}
