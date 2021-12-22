use std::collections::HashMap;
// External uses
use bigdecimal::BigDecimal;
use jsonrpc_core::{BoxFuture, Result};
use jsonrpc_derive::rpc;

// Workspace uses
use zksync_api_types::{
    v02::{
        fee::ApiTxFeeTypes,
        token::ApiNFT,
        transaction::{Toggle2FA, Toggle2FAResponse},
    },
    TxWithSignature,
};
use zksync_crypto::params::ZKSYNC_VERSION;
use zksync_types::{
    tx::{EthBatchSignatures, TxEthSignatureVariant, TxHash},
    AccountId, Address, Fee, Token, TokenId, TokenLike, TotalFee, ZkSyncTx,
};

// Local uses
use super::{types::*, RpcApp};
use crate::fee_ticker::FeeTickerInfo;

pub type BoxFutureResult<T> = BoxFuture<Result<T>>;

macro_rules! spawn {
    ($self: ident.$method: ident($($args: expr),*)) => {{
        let self_ = $self.clone();
        Box::pin(self_.$method($($args),*))
    }}
}

#[rpc]
pub trait Rpc {
    #[rpc(name = "account_info", returns = "AccountInfoResp")]
    fn account_info(&self, addr: Address) -> BoxFutureResult<AccountInfoResp>;

    #[rpc(name = "ethop_info", returns = "ETHOpInfoResp")]
    fn ethop_info(&self, serial_id: u32) -> BoxFutureResult<ETHOpInfoResp>;

    #[rpc(name = "tx_info", returns = "ETHOpInfoResp")]
    fn tx_info(&self, hash: TxHash) -> BoxFutureResult<TransactionInfoResp>;

    #[rpc(name = "tx_submit", returns = "TxHash")]
    fn tx_submit(
        &self,
        tx: Box<ZkSyncTx>,
        signature: Box<TxEthSignatureVariant>,
        fast_processing: Option<bool>,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> BoxFutureResult<TxHash>;

    #[rpc(name = "submit_txs_batch", returns = "Vec<TxHash>")]
    fn submit_txs_batch(
        &self,
        txs: Vec<TxWithSignature>,
        eth_signatures: Option<EthBatchSignatures>,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> BoxFutureResult<Vec<TxHash>>;

    #[rpc(name = "contract_address", returns = "ContractAddressResp")]
    fn contract_address(&self) -> BoxFutureResult<ContractAddressResp>;

    /// "ETH" | #ERC20_ADDRESS => {Token}
    #[rpc(name = "tokens", returns = "Token")]
    fn tokens(&self) -> BoxFutureResult<HashMap<String, Token>>;

    // _address argument is left for the backward compatibility.
    #[rpc(name = "get_tx_fee", returns = "Fee")]
    fn get_tx_fee(
        &self,
        tx_type: ApiTxFeeTypes,
        _address: Address,
        token_like: TokenLike,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> BoxFutureResult<Fee>;

    // _addresses argument is left for the backward compatibility.
    #[rpc(name = "get_txs_batch_fee_in_wei", returns = "TotalFee")]
    fn get_txs_batch_fee_in_wei(
        &self,
        tx_types: Vec<ApiTxFeeTypes>,
        _addresses: Vec<Address>,
        token_like: TokenLike,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> BoxFutureResult<TotalFee>;

    #[rpc(name = "get_token_price", returns = "BigDecimal")]
    fn get_token_price(&self, token_like: TokenLike) -> BoxFutureResult<BigDecimal>;

    #[rpc(name = "get_confirmations_for_eth_op_amount", returns = "u64")]
    fn get_confirmations_for_eth_op_amount(&self) -> BoxFutureResult<u64>;

    #[rpc(name = "get_eth_tx_for_withdrawal", returns = "Option<String>")]
    fn get_eth_tx_for_withdrawal(&self, withdrawal_hash: TxHash)
        -> BoxFutureResult<Option<String>>;

    #[rpc(name = "get_zksync_version", returns = "String")]
    fn get_zksync_version(&self) -> Result<String>;

    #[rpc(name = "get_nft", returns = "Option<ApiNFT>")]
    fn get_nft(&self, id: TokenId) -> BoxFutureResult<Option<ApiNFT>>;

    #[rpc(name = "get_nft_owner", returns = "Option<AccountId>")]
    fn get_nft_owner(&self, id: TokenId) -> BoxFutureResult<Option<AccountId>>;

    #[rpc(name = "toggle_2fa", returns = "Toggle2FAResponse")]
    fn toggle_2fa(&self, toggle_2fa: Toggle2FA) -> BoxFutureResult<Toggle2FAResponse>;

    #[rpc(name = "get_nft_id_by_tx_hash", returns = "Option<TokenId>")]
    fn get_nft_id_by_tx_hash(&self, tx_hash: TxHash) -> BoxFutureResult<Option<TokenId>>;
}

impl<INFO: 'static + FeeTickerInfo + Send + Sync> Rpc for RpcApp<INFO> {
    fn account_info(&self, addr: Address) -> BoxFutureResult<AccountInfoResp> {
        spawn!(self._impl_account_info(addr))
    }

    fn ethop_info(&self, serial_id: u32) -> BoxFutureResult<ETHOpInfoResp> {
        spawn!(self._impl_ethop_info(serial_id))
    }

    fn tx_info(&self, hash: TxHash) -> BoxFutureResult<TransactionInfoResp> {
        spawn!(self._impl_tx_info(hash))
    }

    // Important: the last parameter should have name `meta` and be of type `RequestMetadata`
    fn tx_submit(
        &self,
        tx: Box<ZkSyncTx>,
        signature: Box<TxEthSignatureVariant>,
        fast_processing: Option<bool>,
        meta: Option<RequestMetadata>,
    ) -> BoxFutureResult<TxHash> {
        spawn!(self._impl_tx_submit(tx, signature, fast_processing, meta))
    }

    // Important: the last parameter should have name `meta` and be of type `RequestMetadata`
    fn submit_txs_batch(
        &self,
        txs: Vec<TxWithSignature>,
        eth_signatures: Option<EthBatchSignatures>,
        meta: Option<RequestMetadata>,
    ) -> BoxFutureResult<Vec<TxHash>> {
        spawn!(self._impl_submit_txs_batch(txs, eth_signatures, meta))
    }

    fn contract_address(&self) -> BoxFutureResult<ContractAddressResp> {
        spawn!(self._impl_contract_address())
    }

    fn tokens(&self) -> BoxFutureResult<HashMap<String, Token>> {
        spawn!(self._impl_tokens())
    }

    // Important: the last parameter should have name `meta` and be of type `RequestMetadata`
    fn get_tx_fee(
        &self,
        tx_type: ApiTxFeeTypes,
        address: Address,
        token_like: TokenLike,
        meta: Option<RequestMetadata>,
    ) -> BoxFutureResult<Fee> {
        spawn!(self._impl_get_tx_fee(tx_type, address, token_like, meta))
    }

    // Important: the last parameter should have name `meta` and be of type `RequestMetadata`
    fn get_txs_batch_fee_in_wei(
        &self,
        tx_types: Vec<ApiTxFeeTypes>,
        addresses: Vec<Address>,
        token_like: TokenLike,
        meta: Option<RequestMetadata>,
    ) -> BoxFutureResult<TotalFee> {
        spawn!(self._impl_get_txs_batch_fee_in_wei(tx_types, addresses, token_like, meta))
    }

    fn get_token_price(&self, token_like: TokenLike) -> BoxFutureResult<BigDecimal> {
        spawn!(self._impl_get_token_price(token_like))
    }

    fn get_confirmations_for_eth_op_amount(&self) -> BoxFutureResult<u64> {
        spawn!(self._impl_get_confirmations_for_eth_op_amount())
    }

    fn get_eth_tx_for_withdrawal(
        &self,
        withdrawal_hash: TxHash,
    ) -> BoxFutureResult<Option<String>> {
        spawn!(self._impl_get_eth_tx_for_withdrawal(withdrawal_hash))
    }

    fn get_zksync_version(&self) -> Result<String> {
        Ok(String::from(ZKSYNC_VERSION))
    }

    fn get_nft(&self, id: TokenId) -> BoxFutureResult<Option<ApiNFT>> {
        spawn!(self._impl_get_nft(id))
    }

    fn get_nft_owner(&self, id: TokenId) -> BoxFutureResult<Option<AccountId>> {
        spawn!(self._impl_get_nft_owner(id))
    }

    fn toggle_2fa(&self, toggle_2fa: Toggle2FA) -> BoxFutureResult<Toggle2FAResponse> {
        spawn!(self._impl_toggle_2fa(toggle_2fa))
    }

    fn get_nft_id_by_tx_hash(&self, tx_hash: TxHash) -> BoxFutureResult<Option<TokenId>> {
        spawn!(self._impl_get_nft_id_by_tx_hash(tx_hash))
    }
}
