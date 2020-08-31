use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};
// External uses
use futures::{
    channel::{mpsc, oneshot},
    FutureExt, SinkExt, TryFutureExt,
};
use jsonrpc_core::{Error, ErrorCode, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use num::{BigUint, ToPrimitive};
// Workspace uses
use models::{
    config_options::{ConfigurationOptions, ThreadPanicNotify},
    node::{
        tx::{TxEthSignature, TxHash},
        Account, AccountId, Address, FranklinPriorityOp, FranklinTx, Nonce, PriorityOp, PubKeyHash,
        Token, TokenId, TokenLike, TxFeeTypes,
    },
    primitives::{BigUintSerdeAsRadix10Str, BigUintSerdeWrapper},
};
use storage::{
    chain::{
        block::records::BlockDetails, operations::records::StoredExecutedPriorityOperation,
        operations_ext::records::TxReceiptResponse,
    },
    ConnectionPool, StorageProcessor,
};

// Local uses
use crate::{
    api_server::ops_counter::ChangePubKeyOpsCounter,
    eth_watch::{EthBlockId, EthWatchRequest},
    fee_ticker::{Fee, TickerRequest},
    mempool::{MempoolRequest, TxAddError},
    signature_checker::{VerifiedTx, VerifyTxSignatureRequest},
    state_keeper::StateKeeperRequest,
    utils::{
        current_zksync_info::CurrentZksyncInfo, shared_lru_cache::SharedLruCache,
        token_db_cache::TokenDBCache,
    },
};
use bigdecimal::BigDecimal;
use models::node::tx::EthSignData;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAccountState {
    pub balances: HashMap<String, BigUintSerdeWrapper>,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
}

impl ResponseAccountState {
    pub fn try_restore(account: Account, tokens: &TokenDBCache) -> Result<Self> {
        let mut balances = HashMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            if token_id == 0 {
                balances.insert("ETH".to_string(), balance);
            } else {
                let token = tokens
                    .get_token(token_id)
                    .ok()
                    .flatten()
                    .ok_or_else(Error::internal_error)?;
                balances.insert(token.symbol.clone(), balance);
            }
        }

        Ok(Self {
            balances,
            nonce: account.nonce,
            pub_key_hash: account.pub_key_hash,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingFunds {
    #[serde(with = "BigUintSerdeAsRadix10Str")]
    amount: BigUint,
    expected_accept_block: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DepositingAccountBalances {
    balances: HashMap<String, DepositingFunds>,
}

impl DepositingAccountBalances {
    pub fn from_pending_ops(
        pending_ops: OngoingDepositsResp,
        tokens: &TokenDBCache,
    ) -> Result<Self> {
        let mut balances = HashMap::new();

        for op in pending_ops.deposits {
            let token_symbol = if op.token_id == 0 {
                "ETH".to_string()
            } else {
                tokens
                    .get_token(op.token_id)
                    .map_err(|_| Error::internal_error())?
                    .ok_or_else(Error::internal_error)?
                    .symbol
            };

            let expected_accept_block =
                op.received_on_block + pending_ops.confirmations_for_eth_event;

            let balance = balances
                .entry(token_symbol)
                .or_insert_with(DepositingFunds::default);

            balance.amount += BigUint::from(op.amount);

            // `balance.expected_accept_block` should be the greatest block number among
            // all the deposits for a certain token.
            if expected_accept_block > balance.expected_accept_block {
                balance.expected_accept_block = expected_accept_block;
            }
        }

        Ok(Self { balances })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResp {
    pub address: Address,
    pub id: Option<AccountId>,
    depositing: DepositingAccountBalances,
    pub committed: ResponseAccountState,
    pub verified: ResponseAccountState,
}
