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
use crate::{
    config_options::{ConfigurationOptions, ThreadPanicNotify},
    node::{
        tx::{TxEthSignature, TxHash},
        Account, AccountId, Address, FranklinPriorityOp, FranklinTx, Nonce, PriorityOp, PubKeyHash,
        Token, TokenId, TokenLike, TxFeeTypes,
    },
    primitives::{BigUintSerdeAsRadix10Str, BigUintSerdeWrapper},
};

use crate::node::tx::EthSignData;

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

/// Flattened `PriorityOp` object representing a deposit operation.
/// Used in the `OngoingDepositsResp`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDeposit {
    received_on_block: u64,
    token_id: u16,
    amount: u128,
    eth_tx_hash: String,
}

impl OngoingDeposit {
    pub fn new(received_on_block: u64, priority_op: PriorityOp) -> Self {
        let (token_id, amount) = match priority_op.data {
            FranklinPriorityOp::Deposit(deposit) => (
                deposit.token,
                deposit
                    .amount
                    .to_u128()
                    .expect("Deposit amount should be less then u128::max()"),
            ),
            other => {
                panic!("Incorrect input for OngoingDeposit: {:?}", other);
            }
        };

        let eth_tx_hash = hex::encode(&priority_op.eth_hash);

        Self {
            received_on_block,
            token_id,
            amount,
            eth_tx_hash,
        }
    }
}

/// Information about ongoing deposits for certain recipient address.
///
/// Please note that since this response is based on the events that are
/// currently awaiting confirmations, this information is approximate:
/// blocks on Ethereum can be reverted, and final list of executed deposits
/// can differ from the this estimation.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDepositsResp {
    /// Address for which response is served.
    address: Address,
    /// List of tuples (Eth block number, Deposit operation) of ongoing
    /// deposit operations.
    deposits: Vec<OngoingDeposit>,

    /// Amount of confirmations required for every deposit to be processed.
    confirmations_for_eth_event: u64,

    /// Estimated block number for deposits completions:
    /// all the deposit operations for provided address are expected to be
    /// accepted in the zkSync network upon reaching this blocks.
    ///
    /// Can be `None` if there are no ongoing deposits.
    estimated_deposits_approval_block: Option<u64>,
}
