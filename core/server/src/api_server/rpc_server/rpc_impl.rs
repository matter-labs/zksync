use std::collections::HashMap;
// External uses
use futures::{channel::oneshot, SinkExt};
use jsonrpc_core::{Error, Result};
use num::BigUint;
// Workspace uses
use models::node::{
    tx::{TxEthSignature, TxHash},
    Address, FranklinTx, Token, TokenLike, TxFeeTypes,
};

// Local uses
use crate::{
    fee_ticker::Fee,
    mempool::{MempoolRequest, TxAddError},
    state_keeper::StateKeeperRequest,
};
use bigdecimal::BigDecimal;

use super::{error::*, types::*, verify_tx_info_message_signature, RpcApp};

impl RpcApp {
    pub async fn _impl_account_info(self, address: Address) -> Result<AccountInfoResp> {
        // TODO: this method now has a lot debug output, to be removed as soon as problem is detected.
        use std::time::Instant;

        let started = Instant::now();
        let mut state_keeper_request_sender = self.state_keeper_request_sender.clone();

        let state_keeper_response = oneshot::channel();
        state_keeper_request_sender
            .send(StateKeeperRequest::GetAccount(
                address,
                state_keeper_response.0,
            ))
            .await
            .map_err(|err| {
                log::warn!(
                    "[{}:{}:{}] Internal Server Error: '{}'; input: {}",
                    file!(),
                    line!(),
                    column!(),
                    err,
                    address,
                );
                Error::internal_error()
            })?;

        let committed_account_state = state_keeper_response.1.await.map_err(|err| {
            log::warn!(
                "[{}:{}:{}] Internal Server Error: '{}'; input: {}",
                file!(),
                line!(),
                column!(),
                err,
                address,
            );
            Error::internal_error()
        })?;

        let (id, committed) = if let Some((id, account)) = committed_account_state {
            let restored_state =
                ResponseAccountState::try_restore(account, &self.token_cache).await?;
            (Some(id), restored_state)
        } else {
            (None, Default::default())
        };

        let verified = self.get_verified_account_state(&address).await?;

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
            id,
            committed,
            verified,
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
        mut tx: Box<FranklinTx>,
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

        let fast_processing = fast_processing.unwrap_or_default(); // `None` => false

        if fast_processing && !tx.is_withdraw() {
            return Err(Error {
                code: RpcErrorCodes::UnsupportedFastProcessing.into(),
                message: "Fast processing available only for 'withdraw' operation type."
                    .to_string(),
                data: None,
            });
        }

        if let FranklinTx::Withdraw(withdraw) = tx.as_mut() {
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

        let tx_fee_info = match tx.as_ref() {
            FranklinTx::Withdraw(withdraw) => {
                let fee_type = if fast_processing {
                    TxFeeTypes::FastWithdraw
                } else {
                    TxFeeTypes::Withdraw
                };

                Some((
                    fee_type,
                    TokenLike::Id(withdraw.token),
                    withdraw.to,
                    withdraw.fee.clone(),
                ))
            }
            FranklinTx::Transfer(transfer) => Some((
                TxFeeTypes::Transfer,
                TokenLike::Id(transfer.token),
                transfer.to,
                transfer.fee.clone(),
            )),
            _ => None,
        };

        let mut mempool_sender = self.mempool_request_sender.clone();
        let sign_verify_channel = self.sign_verify_request_sender.clone();
        let ticker_request_sender = self.ticker_request_sender.clone();
        let ops_counter = self.ops_counter.clone();

        if let Some((tx_type, token, address, provided_fee)) = tx_fee_info {
            let required_fee =
                Self::ticker_request(ticker_request_sender, tx_type, address, token.clone())
                    .await?;
            // We allow fee to be 5% off the required fee
            let scaled_provided_fee =
                provided_fee.clone() * BigUint::from(105u32) / BigUint::from(100u32);
            if required_fee.total_fee >= scaled_provided_fee {
                warn!(
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
        .await?;

        // Check whether operations limit for this account was reached.
        // We must do it after we've checked that transaction is correct to avoid the situation
        // when somebody sends incorrect transactions to deny changing the pubkey for some account ID.
        if let FranklinTx::ChangePubKey(tx) = tx.as_ref() {
            let mut ops_counter_lock = ops_counter.write().expect("Write lock");

            if let Err(error) = ops_counter_lock.check_allowanse(&tx) {
                return Err(Error {
                    code: RpcErrorCodes::OperationsLimitReached.into(),
                    message: error.to_string(),
                    data: None,
                });
            }
        }

        let hash = tx.hash();
        let mempool_resp = oneshot::channel();
        mempool_sender
                .send(MempoolRequest::NewTx(Box::new(verified_tx), mempool_resp.0))
                .await
                .map_err(|err| {
                    log::warn!(
                        "[{}:{}:{}] Internal Server Error: '{}'; input: <Tx: '{:?}', signature: '{:?}'>",
                        file!(),
                        line!(),
                        column!(),
                        err,
                        tx,
                        signature,
                    );
                    Error::internal_error()
                })?;
        let tx_add_result = mempool_resp.1.await.unwrap_or(Err(TxAddError::Other));

        tx_add_result.map(|_| hash).map_err(|e| Error {
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

    pub async fn _impl_get_token_price(self, token: TokenLike) -> Result<BigDecimal> {
        Self::ticker_price_request(self.ticker_request_sender.clone(), token).await
    }
}
