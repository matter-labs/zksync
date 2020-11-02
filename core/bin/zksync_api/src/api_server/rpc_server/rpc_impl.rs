use std::collections::HashMap;
// External uses
use jsonrpc_core::{Error, Result};
use num::{bigint::ToBigInt, BigUint};
// Workspace uses
use zksync_types::{
    helpers::closest_packable_fee_amount,
    tx::{TxEthSignature, TxHash},
    Address, Token, TokenLike, TxFeeTypes, ZkSyncTx,
};

// Local uses
use crate::{
    fee_ticker::{BatchFee, Fee, TokenPriceRequestType},
    tx_error::TxAddError,
};
use bigdecimal::BigDecimal;

use super::{error::*, types::*, verify_tx_info_message_signature, RpcApp};

impl RpcApp {
    pub async fn _impl_account_info(self, address: Address) -> Result<AccountInfoResp> {
        use std::time::Instant;

        let started = Instant::now();

        let account_state = self.get_account_state(&address).await?;

        let depositing_ops = self.get_ongoing_deposits_impl(address).await?;
        let depositing =
            DepositingAccountBalances::from_pending_ops(depositing_ops, &self.token_cache).await?;

        log::trace!(
            "account_info: address {}, total request processing {}ms",
            &address,
            started.elapsed().as_millis()
        );

        Ok(AccountInfoResp {
            address,
            id: account_state.account_id,
            committed: account_state.committed,
            verified: account_state.verified,
            depositing,
        })
    }

    pub async fn _impl_ethop_info(self, serial_id: u32) -> Result<ETHOpInfoResp> {
        let executed_op = self.get_executed_priority_operation(serial_id).await?;
        Ok(if let Some(executed_op) = executed_op {
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
        })
    }

    pub async fn _impl_get_confirmations_for_eth_op_amount(self) -> Result<u64> {
        Ok(self.confirmations_for_eth_event)
    }

    pub async fn _impl_tx_info(self, tx_hash: TxHash) -> Result<TransactionInfoResp> {
        let stored_receipt = self.get_tx_receipt(tx_hash).await?;
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

    pub async fn _impl_tx_submit(
        self,
        mut tx: Box<ZkSyncTx>,
        signature: Box<Option<TxEthSignature>>,
        fast_processing: Option<bool>,
    ) -> Result<TxHash> {
        if tx.is_close() {
            return Err(Error {
                code: RpcErrorCodes::AccountCloseDisabled.into(),
                message: "Account close tx is disabled.".to_string(),
                data: None,
            });
        }

        if let ZkSyncTx::ForcedExit(forced_exit) = &*tx {
            self.check_forced_exit(&forced_exit).await?;
        }

        let fast_processing = fast_processing.unwrap_or_default(); // `None` => false

        if fast_processing && !tx.is_withdraw() {
            return Err(Error {
                code: RpcErrorCodes::UnsupportedFastProcessing.into(),
                message: "Fast processing available only for 'withdraw' operation type."
                    .to_string(),
                data: None,
            });
        }

        if let ZkSyncTx::Withdraw(withdraw) = tx.as_mut() {
            if withdraw.fast {
                // We set `fast` field ourselves, so we have to check that user did not set it themselves.
                return Err(Error {
                    code: RpcErrorCodes::IncorrectTx.into(),
                    message: "'fast' field of Withdraw transaction must not be set manually."
                        .to_string(),
                    data: None,
                });
            }

            // `fast` field is not used in serializing (as it's an internal server option,
            // not the actual transaction part), so we have to set it manually depending on
            // the RPC method input.
            withdraw.fast = fast_processing;
        }

        let msg_to_sign = self.get_tx_info_message_to_sign(&tx).await?;

        let tx_fee_info = tx.get_fee_info();

        let sign_verify_channel = self.sign_verify_request_sender.clone();
        let ticker_request_sender = self.ticker_request_sender.clone();

        if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
            let should_enforce_fee =
                !matches!(tx_type, TxFeeTypes::ChangePubKey{..}) || self.enforce_pubkey_change_fee;

            let required_fee =
                Self::ticker_request(ticker_request_sender, tx_type, address, token.clone())
                    .await?;
            // We allow fee to be 5% off the required fee
            let scaled_provided_fee =
                provided_fee.clone() * BigUint::from(105u32) / BigUint::from(100u32);
            if required_fee.total_fee >= scaled_provided_fee && should_enforce_fee {
                vlog::warn!(
                    "User provided fee is too low, required: {:?}, provided: {} (scaled: {}), token: {:?}",
                    required_fee, provided_fee, scaled_provided_fee, token
                );
                return Err(Error {
                    code: RpcErrorCodes::from(TxAddError::TxFeeTooLow).into(),
                    message: TxAddError::TxFeeTooLow.to_string(),
                    data: None,
                });
            }
        }

        let verified_tx = verify_tx_info_message_signature(
            &tx,
            *signature.clone(),
            msg_to_sign,
            sign_verify_channel,
        )
        .await?
        .into_inner();

        let hash = tx.hash();

        // Send verified transactions to the mempool.
        let tx_add_result = self
            .api_client
            .send_tx(verified_tx)
            .await
            .map_err(|_| Error {
                code: RpcErrorCodes::Other.into(),
                message: "Error communicating core server".into(),
                data: None,
            })?;

        // Check the mempool response and, if everything is OK, return the transactions hashes.
        tx_add_result.map(|_| hash).map_err(|e| Error {
            code: RpcErrorCodes::from(e).into(),
            message: e.to_string(),
            data: None,
        })
    }

    pub async fn _impl_submit_txs_batch(self, txs: Vec<TxWithSignature>) -> Result<Vec<TxHash>> {
        if txs.is_empty() {
            return Err(Error {
                code: RpcErrorCodes::from(TxAddError::EmptyBatch).into(),
                message: "Transaction batch cannot be empty".to_string(),
                data: None,
            });
        }

        for tx in &txs {
            if tx.tx.is_close() {
                return Err(Error {
                    code: RpcErrorCodes::AccountCloseDisabled.into(),
                    message: "Account close tx is disabled.".to_string(),
                    data: None,
                });
            }
        }
        let mut messages_to_sign = vec![];
        for tx in &txs {
            messages_to_sign.push(self.get_tx_info_message_to_sign(&tx.tx).await?);
        }

        // Checking fees data
        let mut required_total_usd_fee = BigDecimal::from(0);
        let mut provided_total_usd_fee = BigDecimal::from(0);
        for tx in &txs {
            let tx_fee_info = match &tx.tx {
                // Cause `ChangePubKey` will have fee we must add this check
                // TODO: should be removed after merging with a branch that contains a fee on ChangePubKey
                ZkSyncTx::ChangePubKey(_) => {
                    // Now `ChangePubKey` operations are not allowed in batches
                    return Err(Error {
                        code: RpcErrorCodes::from(TxAddError::Other).into(),
                        message: "ChangePubKey operations are not allowed in batches".to_string(),
                        data: None,
                    });
                }
                _ => tx.tx.get_fee_info(),
            };
            if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
                let required_fee = Self::ticker_request(
                    self.ticker_request_sender.clone(),
                    tx_type,
                    address,
                    token.clone(),
                )
                .await?;
                let token_price_in_usd = Self::ticker_price_request(
                    self.ticker_request_sender.clone(),
                    token.clone(),
                    TokenPriceRequestType::USDForOneWei,
                )
                .await?;
                required_total_usd_fee +=
                    BigDecimal::from(required_fee.total_fee.to_bigint().unwrap())
                        * &token_price_in_usd;
                provided_total_usd_fee +=
                    BigDecimal::from(provided_fee.clone().to_bigint().unwrap())
                        * &token_price_in_usd;
            }
        }
        // We allow fee to be 5% off the required fee
        let scaled_provided_fee_in_usd =
            provided_total_usd_fee.clone() * BigDecimal::from(105u32) / BigDecimal::from(100u32);
        if required_total_usd_fee >= scaled_provided_fee_in_usd {
            return Err(Error {
                code: RpcErrorCodes::from(TxAddError::TxBatchFeeTooLow).into(),
                message: TxAddError::TxBatchFeeTooLow.to_string(),
                data: None,
            });
        }

        let mut verified_txs = Vec::new();
        for (tx, msg_to_sign) in txs.iter().zip(messages_to_sign.iter()) {
            let verified_tx = verify_tx_info_message_signature(
                &tx.tx,
                tx.signature.clone(),
                msg_to_sign.clone(),
                self.sign_verify_request_sender.clone(),
            )
            .await?;

            verified_txs.push(verified_tx.into_inner());
        }

        let tx_hashes: Vec<TxHash> = txs.iter().map(|tx| tx.tx.hash()).collect();

        // Send verified transactions to the mempool.
        let tx_add_result = self
            .api_client
            .send_txs_batch(verified_txs)
            .await
            .map_err(|_| Error {
                code: RpcErrorCodes::Other.into(),
                message: "Error communicating core server".into(),
                data: None,
            })?;

        // Check the mempool response and, if everything is OK, return the transactions hashes.
        tx_add_result.map(|_| tx_hashes).map_err(|e| Error {
            code: RpcErrorCodes::from(e).into(),
            message: e.to_string(),
            data: None,
        })
    }

    pub async fn _impl_contract_address(self) -> Result<ContractAddressResp> {
        let mut storage = self.access_storage().await?;
        let config = storage.config_schema().load_config().await.map_err(|err| {
            log::warn!(
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
        Ok(ContractAddressResp {
            main_contract,
            gov_contract,
        })
    }

    pub async fn _impl_tokens(self) -> Result<HashMap<String, Token>> {
        let mut storage = self.access_storage().await?;
        let mut tokens = storage.tokens_schema().load_tokens().await.map_err(|err| {
            log::warn!(
                "[{}:{}:{}] Internal Server Error: '{}'; input: N/A",
                file!(),
                line!(),
                column!(),
                err
            );
            Error::internal_error()
        })?;
        Ok(tokens
            .drain()
            .map(|(id, token)| {
                if id == 0 {
                    ("ETH".to_string(), token)
                } else {
                    (token.symbol.clone(), token)
                }
            })
            .collect())
    }

    pub async fn _impl_get_tx_fee(
        self,
        tx_type: TxFeeTypes,
        address: Address,
        token: TokenLike,
    ) -> Result<Fee> {
        Self::ticker_request(self.ticker_request_sender.clone(), tx_type, address, token).await
    }

    pub async fn _impl_get_txs_batch_fee_in_wei(
        self,
        tx_types: Vec<TxFeeTypes>,
        addresses: Vec<Address>,
        token: TokenLike,
    ) -> Result<BatchFee> {
        if tx_types.len() != addresses.len() {
            return Err(Error {
                code: RpcErrorCodes::IncorrectTx.into(),
                message: "Number of tx_types must be equal to the number of addresses".to_string(),
                data: None,
            });
        }

        let ticker_request_sender = self.ticker_request_sender.clone();

        let mut total_fee = BigUint::from(0u32);

        for (tx_type, address) in tx_types.iter().zip(addresses.iter()) {
            total_fee += Self::ticker_request(
                ticker_request_sender.clone(),
                tx_type.clone(),
                *address,
                token.clone(),
            )
            .await?
            .total_fee;
        }
        // Sum of transactions can be unpackable
        total_fee = closest_packable_fee_amount(&total_fee);

        Ok(BatchFee { total_fee })
    }

    pub async fn _impl_get_token_price(self, token: TokenLike) -> Result<BigDecimal> {
        Self::ticker_price_request(
            self.ticker_request_sender.clone(),
            token,
            TokenPriceRequestType::USDForOneToken,
        )
        .await
    }

    pub async fn _impl_get_eth_tx_for_withdrawal(
        self,
        withdrawal_hash: TxHash,
    ) -> Result<Option<String>> {
        self.eth_tx_for_withdrawal(withdrawal_hash).await
    }
}
