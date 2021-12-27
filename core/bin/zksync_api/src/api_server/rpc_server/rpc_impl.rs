use std::collections::HashMap;
use std::time::Instant;
// External uses
use bigdecimal::BigDecimal;
use jsonrpc_core::{Error, Result};
// Workspace uses
use zksync_api_types::{
    v02::{
        fee::ApiTxFeeTypes,
        token::ApiNFT,
        transaction::{Toggle2FA, Toggle2FAResponse},
    },
    TxWithSignature,
};
use zksync_crypto::params::MIN_NFT_TOKEN_ID;
use zksync_types::{
    tx::{EthBatchSignatures, TxEthSignatureVariant, TxHash},
    AccountId, Address, Fee, Token, TokenId, TokenLike, TotalFee, TxFeeTypes, ZkSyncTx,
};
// Local uses
use crate::{
    api_server::{
        helpers::get_depositing, rpc_server::error::RpcErrorCodes, tx_sender::SubmitError,
    },
    fee_ticker::TokenPriceRequestType,
};

use super::{types::*, RpcApp};

impl RpcApp {
    pub async fn _impl_account_info(self, address: Address) -> Result<AccountInfoResp> {
        let start = Instant::now();

        let account_state = self.get_account_state(address).await?;

        let mut storage = self.access_storage().await?;
        let depositing = get_depositing(
            &mut storage,
            &self.tx_sender.core_api_client,
            &self.tx_sender.tokens,
            address,
            self.confirmations_for_eth_event,
        )
        .await
        .map_err(|_| Error::internal_error())?;
        let account_type = if let Some(account_id) = account_state.account_id {
            storage
                .chain()
                .account_schema()
                .account_type_by_id(account_id)
                .await
                .map_err(|_| Error::internal_error())?
                .map(|t| t.into())
        } else {
            None
        };

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "account_info");
        Ok(AccountInfoResp {
            address,
            id: account_state.account_id,
            committed: account_state.committed,
            verified: account_state.verified,
            depositing,
            account_type,
        })
    }

    pub async fn _impl_ethop_info(self, serial_id: u32) -> Result<ETHOpInfoResp> {
        let start = Instant::now();
        let executed_op = self.get_executed_priority_operation(serial_id).await?;
        let result = if let Some(executed_op) = executed_op {
            let block = self.get_block_info(executed_op.block_number).await?;
            ETHOpInfoResp {
                executed: true,
                block: Some(BlockInfo {
                    block_number: executed_op.block_number,
                    committed: true,
                    verified: block.map(|b| b.verified_at.is_some()).unwrap_or_default(),
                }),
            }
        } else {
            ETHOpInfoResp {
                executed: false,
                block: None,
            }
        };

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "ethop_info");
        Ok(result)
    }

    pub async fn _impl_get_confirmations_for_eth_op_amount(self) -> Result<u64> {
        Ok(self.confirmations_for_eth_event)
    }

    pub async fn _impl_tx_info(self, tx_hash: TxHash) -> Result<TransactionInfoResp> {
        let start = Instant::now();
        let stored_receipt = self.get_tx_receipt(tx_hash).await?;
        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "tx_info");
        Ok(if let Some(stored_receipt) = stored_receipt {
            TransactionInfoResp {
                executed: true,
                success: Some(stored_receipt.success),
                fail_reason: stored_receipt.fail_reason,
                block: Some(BlockInfo {
                    block_number: stored_receipt.block_number,
                    committed: true,
                    verified: stored_receipt.verified,
                }),
            }
        } else {
            TransactionInfoResp {
                executed: false,
                success: None,
                fail_reason: None,
                block: None,
            }
        })
    }

    #[allow(deprecated)]
    pub async fn _impl_tx_submit(
        self,
        tx: Box<ZkSyncTx>,
        signature: Box<TxEthSignatureVariant>,
        fast_processing: Option<bool>,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<TxHash> {
        let start = Instant::now();

        let result = self
            .tx_sender
            .submit_tx_with_separate_fp(
                *tx,
                *signature,
                fast_processing,
                extracted_request_metadata,
            )
            .await;
        if let Err(err) = &result {
            let err_label = match err {
                SubmitError::IncorrectTx(err) => err.clone(),
                SubmitError::TxAdd(err) => err.to_string(),
                _ => "other".to_string(),
            };
            let labels = vec![("stage", "api".to_string()), ("error", err_label)];
            metrics::increment_counter!("rejected_txs", &labels);
        }

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "tx_submit");
        result.map_err(Error::from)
    }

    pub async fn _impl_submit_txs_batch(
        self,
        txs: Vec<TxWithSignature>,
        eth_signatures: Option<EthBatchSignatures>,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<Vec<TxHash>> {
        let start = Instant::now();

        let result = self
            .tx_sender
            .submit_txs_batch(txs, eth_signatures, extracted_request_metadata)
            .await
            .map(|response| {
                response
                    .transaction_hashes
                    .into_iter()
                    .map(|tx_hash| tx_hash.0)
                    .collect()
            });

        if let Err(err) = &result {
            let err_label = match err {
                SubmitError::IncorrectTx(err) => err.clone(),
                SubmitError::TxAdd(err) => err.to_string(),
                _ => "other".to_string(),
            };
            let labels = vec![("stage", "api".to_string()), ("error", err_label)];
            metrics::increment_counter!("rejected_txs", &labels);
        }

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "submit_txs_batch");
        result.map_err(Error::from)
    }

    pub async fn _impl_contract_address(self) -> Result<ContractAddressResp> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let config = storage.config_schema().load_config().await.map_err(|err| {
            vlog::warn!(
                "[{}:{}:{}] Internal Server Error: '{}'; input: N/A",
                file!(),
                line!(),
                column!(),
                err
            );
            Error::internal_error()
        })?;

        // `expect` calls below are safe, since not having the addresses in the server config
        // means a misconfiguration, server cannot operate in this condition.
        let main_contract = config
            .contract_addr
            .expect("Server config doesn't contain the main contract address");
        let gov_contract = config
            .gov_contract_addr
            .expect("Server config doesn't contain the gov contract address");

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "contract_address");
        Ok(ContractAddressResp {
            main_contract,
            gov_contract,
        })
    }

    pub async fn _impl_get_nft(self, id: TokenId) -> Result<Option<ApiNFT>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let nft = storage
            .tokens_schema()
            .get_nft_with_factories(id)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: N/A", err);
                Error::internal_error()
            })?;

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_nft");
        Ok(nft)
    }

    pub async fn _impl_tokens(self) -> Result<HashMap<String, Token>> {
        let start = Instant::now();
        let mut storage = self.access_storage().await?;
        let mut tokens = storage.tokens_schema().load_tokens().await.map_err(|err| {
            vlog::warn!("Internal Server Error: '{}'; input: N/A", err);
            Error::internal_error()
        })?;

        let result: HashMap<_, _> = tokens
            .drain()
            .map(|(id, token)| {
                if *id == 0 {
                    ("ETH".to_string(), token)
                } else {
                    (token.symbol.clone(), token)
                }
            })
            .collect();

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "tokens");
        Ok(result)
    }

    pub async fn _impl_get_tx_fee(
        self,
        tx_type: ApiTxFeeTypes,
        address: Address,
        token: TokenLike,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<Fee> {
        let start = Instant::now();
        let token_allowed =
            Self::token_allowed_for_fees(&self.tx_sender.ticker, token.clone()).await?;
        if !token_allowed {
            return Err(SubmitError::InappropriateFeeToken.into());
        }

        let result = Self::ticker_request(
            &self.tx_sender.ticker,
            tx_type.into(),
            address,
            token.clone(),
        )
        .await?;

        let should_subsidize_cpk = self
            .tx_sender
            .should_subsidize_cpk(
                &result.normal_fee.total_fee,
                &result.subsidized_fee.total_fee,
                &result.subsidy_size_usd,
                extracted_request_metadata,
            )
            .await?;

        let fee = if should_subsidize_cpk {
            result.subsidized_fee
        } else {
            result.normal_fee
        };

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_tx_fee");
        Ok(fee)
    }

    pub async fn _impl_get_txs_batch_fee_in_wei(
        self,
        tx_types: Vec<ApiTxFeeTypes>,
        addresses: Vec<Address>,
        token: TokenLike,
        extracted_request_metadata: Option<RequestMetadata>,
    ) -> Result<TotalFee> {
        let start = Instant::now();
        if tx_types.len() != addresses.len() {
            return Err(Error {
                code: RpcErrorCodes::IncorrectTx.into(),
                message: "Number of tx_types must be equal to the number of addresses".to_string(),
                data: None,
            });
        }

        let token_allowed =
            Self::token_allowed_for_fees(&self.tx_sender.ticker, token.clone()).await?;
        if !token_allowed {
            return Err(SubmitError::InappropriateFeeToken.into());
        }

        let transactions: Vec<(TxFeeTypes, Address)> = (tx_types
            .iter()
            .cloned()
            .map(|fee_type| fee_type.into())
            .zip(addresses.iter().cloned()))
        .collect();

        let result =
            Self::ticker_batch_fee_request(&self.tx_sender.ticker, transactions, token.clone())
                .await?;

        let should_subsidize_cpk = self
            .tx_sender
            .should_subsidize_cpk(
                &result.normal_fee.total_fee,
                &result.subsidized_fee.total_fee,
                &result.subsidy_size_usd,
                extracted_request_metadata,
            )
            .await?;

        let fee = if should_subsidize_cpk {
            result.subsidized_fee
        } else {
            result.normal_fee
        };

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_txs_batch_fee_in_wei");
        Ok(TotalFee {
            total_fee: fee.total_fee,
        })
    }

    pub async fn _impl_get_token_price(self, token: TokenLike) -> Result<BigDecimal> {
        let start = Instant::now();
        let result = Self::ticker_price_request(
            &self.tx_sender.ticker,
            token,
            TokenPriceRequestType::USDForOneToken,
        )
        .await;
        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_token_price");
        result
    }

    pub async fn _impl_get_eth_tx_for_withdrawal(
        self,
        withdrawal_hash: TxHash,
    ) -> Result<Option<String>> {
        let start = Instant::now();
        let result = self.eth_tx_for_withdrawal(withdrawal_hash).await;
        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_eth_tx_for_withdrawal");
        result
    }

    pub async fn _impl_get_nft_owner(self, id: TokenId) -> Result<Option<AccountId>> {
        let start = Instant::now();
        let owner_id = if id.0 < MIN_NFT_TOKEN_ID {
            None
        } else {
            let mut storage = self.access_storage().await?;
            storage
                .chain()
                .account_schema()
                .get_nft_owner(id)
                .await
                .map_err(|err| {
                    vlog::warn!("Internal Server Error: '{}'; input: N/A", err);
                    Error::internal_error()
                })?
        };

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_nft_owner");
        Ok(owner_id)
    }

    pub async fn _impl_toggle_2fa(self, toggle_2fa: Toggle2FA) -> Result<Toggle2FAResponse> {
        let start = Instant::now();
        let response = self
            .tx_sender
            .toggle_2fa(toggle_2fa)
            .await
            .map_err(Error::from);

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "toggle_2fa");
        response
    }

    pub async fn _impl_get_nft_id_by_tx_hash(self, tx_hash: TxHash) -> Result<Option<TokenId>> {
        let start = Instant::now();

        let mut storage = self.access_storage().await?;
        let response = storage
            .chain()
            .state_schema()
            .get_nft_id_by_tx_hash(tx_hash)
            .await
            .map_err(|err| {
                vlog::warn!("Internal Server Error: '{}'; input: N/A", err);
                Error::internal_error()
            })?;

        metrics::histogram!("api", start.elapsed(), "type" => "rpc", "endpoint_name" => "get_nft_id_by_tx_hash");
        Ok(response)
    }
}
