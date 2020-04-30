use crate::eth_watch::EthWatchRequest;
use crate::mempool::MempoolRequest;
use crate::mempool::TxAddError;
use crate::state_keeper::StateKeeperRequest;
use bigdecimal::BigDecimal;
use futures::channel::{mpsc, oneshot};
use futures::{FutureExt, SinkExt, TryFutureExt};
use jsonrpc_core::{Error, ErrorCode, Result};
use jsonrpc_core::{IoHandler, MetaIoHandler, Metadata, Middleware};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use models::config_options::{ConfigurationOptions, ThreadPanicNotify};
use models::node::tx::TxEthSignature;
use models::node::tx::TxHash;
use models::node::{
    closest_packable_fee_amount, Account, AccountId, FranklinPriorityOp, FranklinTx, Nonce,
    PriorityOp, PubKeyHash, Token, TokenId, TokenLike,
};
use models::primitives::{big_decimal_to_u128, floor_big_decimal};
use std::collections::HashMap;
use std::convert::TryInto;
use storage::{ConnectionPool, StorageProcessor};
use web3::types::Address;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAccountState {
    balances: HashMap<String, BigDecimal>,
    nonce: Nonce,
    pub_key_hash: PubKeyHash,
}

impl ResponseAccountState {
    pub fn try_to_restore(account: Account, tokens: &HashMap<TokenId, Token>) -> Result<Self> {
        let mut balances = HashMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            if token_id == 0 {
                balances.insert("ETH".to_string(), balance);
            } else {
                let token = tokens.get(&token_id).ok_or_else(Error::internal_error)?;
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
    amount: BigDecimal,
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
        tokens: &HashMap<TokenId, Token>,
    ) -> Result<Self> {
        let mut balances = HashMap::new();

        for op in pending_ops.deposits {
            let token_symbol = if op.token_id == 0 {
                "ETH".to_string()
            } else {
                tokens
                    .get(&op.token_id)
                    .ok_or_else(Error::internal_error)?
                    .symbol
                    .clone()
            };

            let expected_accept_block =
                op.received_on_block + pending_ops.confirmations_for_eth_event;

            let balance = balances
                .entry(token_symbol)
                .or_insert(DepositingFunds::default());

            balance.amount += BigDecimal::from(op.amount);
            if balance.expected_accept_block < expected_accept_block {
                balance.expected_accept_block = expected_accept_block;
            }
        }

        Ok(Self { balances })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResp {
    address: Address,
    id: Option<AccountId>,
    depositing: DepositingAccountBalances,
    committed: ResponseAccountState,
    verified: ResponseAccountState,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockInfo {
    pub block_number: i64,
    pub committed: bool,
    pub verified: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TransactionInfoResp {
    pub executed: bool,
    pub success: Option<bool>,
    pub fail_reason: Option<String>,
    pub block: Option<BlockInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ETHOpInfoResp {
    pub executed: bool,
    pub block: Option<BlockInfo>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContractAddressResp {
    pub main_contract: String,
    pub gov_contract: String,
}

/// Flattened `PriorityOp` object representing a deposit operation.
/// Used in the `OngoingDepositsResp`.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OngoingDeposit {
    received_on_block: u64,
    token_id: u16,
    amount: u64,
    eth_tx_hash: String,
}

impl OngoingDeposit {
    pub fn new(received_on_block: u64, priority_op: PriorityOp) -> Self {
        let (token_id, amount) = match priority_op.data {
            FranklinPriorityOp::Deposit(deposit) => (
                deposit.token,
                big_decimal_to_u128(&deposit.amount)
                    .try_into()
                    .expect("Too big deposit amount"),
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum TxFeeTypes {
    Withdraw,
    Transfer,
}

enum RpcErrorCodes {
    NonceMismatch = 101,
    IncorrectTx = 103,

    MissingEthSignature = 200,
    EIP1271SignatureVerificationFail = 201,
    IncorrectEthSignature = 202,
    ChangePkNotAuthorized = 203,

    Other = 300,
    AccountCloseDisabled = 301,
}

impl From<TxAddError> for RpcErrorCodes {
    fn from(error: TxAddError) -> Self {
        match error {
            TxAddError::NonceMismatch => Self::NonceMismatch,
            TxAddError::IncorrectTx => Self::IncorrectTx,
            TxAddError::MissingEthSignature => Self::MissingEthSignature,
            TxAddError::EIP1271SignatureVerificationFail => Self::EIP1271SignatureVerificationFail,
            TxAddError::IncorrectEthSignature => Self::IncorrectEthSignature,
            TxAddError::ChangePkNotAuthorized => Self::ChangePkNotAuthorized,
            TxAddError::Other => Self::Other,
        }
    }
}

impl Into<ErrorCode> for RpcErrorCodes {
    fn into(self) -> ErrorCode {
        (self as i64).into()
    }
}

#[rpc]
pub trait Rpc {
    #[rpc(name = "account_info", returns = "AccountInfoResp")]
    fn account_info(
        &self,
        addr: Address,
    ) -> Box<dyn futures01::Future<Item = AccountInfoResp, Error = Error> + Send>;
    #[rpc(name = "ethop_info")]
    fn ethop_info(&self, serial_id: u32) -> Result<ETHOpInfoResp>;
    #[rpc(name = "tx_info")]
    fn tx_info(&self, hash: TxHash) -> Result<TransactionInfoResp>;
    #[rpc(name = "tx_submit", returns = "TxHash")]
    fn tx_submit(
        &self,
        tx: Box<FranklinTx>,
        signature: Box<Option<TxEthSignature>>,
    ) -> Box<dyn futures01::Future<Item = TxHash, Error = Error> + Send>;
    #[rpc(name = "contract_address")]
    fn contract_address(&self) -> Result<ContractAddressResp>;
    /// "ETH" | #ERC20_ADDRESS => {Token}
    #[rpc(name = "tokens")]
    fn tokens(&self) -> Result<HashMap<String, Token>>;
    #[rpc(name = "get_tx_fee")]
    fn get_tx_fee(
        &self,
        tx_type: TxFeeTypes,
        amount: BigDecimal,
        token_like: TokenLike,
    ) -> Result<BigDecimal>;

    #[rpc(name = "get_ongoing_deposits", returns = "OngoingDepositsResp")]
    fn get_ongoing_deposits(
        &self,
        addr: Address,
    ) -> Box<dyn futures01::Future<Item = OngoingDepositsResp, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct RpcApp {
    pub mempool_request_sender: mpsc::Sender<MempoolRequest>,
    pub state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    pub eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
    pub connection_pool: ConnectionPool,

    pub confirmations_for_eth_event: u64,
}

impl RpcApp {
    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }
}

impl RpcApp {
    fn access_storage(&self) -> Result<StorageProcessor> {
        self.connection_pool
            .access_storage_fragile()
            .map_err(|_| Error::internal_error())
    }

    /// Async version of `get_ongoing_deposits` which does not use old futures as a return type.
    async fn get_ongoing_deposits_impl(&self, address: Address) -> Result<OngoingDepositsResp> {
        let mut eth_watcher_request_sender = self.eth_watcher_request_sender.clone();
        let confirmations_for_eth_event = self.confirmations_for_eth_event;

        let eth_watcher_response = oneshot::channel();

        // Get all the ongoing priority ops from the `EthWatcher`.
        eth_watcher_request_sender
            .send(EthWatchRequest::GetUnconfirmedQueueOps {
                resp: eth_watcher_response.0,
            })
            .await
            .map_err(|_| Error::internal_error())?;

        let ongoing_ops = eth_watcher_response
            .1
            .await
            .map_err(|_| Error::internal_error())?;

        let mut max_block_number = 0;

        // Filter only deposits for the requested address.
        // `map` is used after filter to find the max block number without an
        // additional list pass.
        let deposits: Vec<_> = ongoing_ops
            .into_iter()
            .filter(|(_block, op)| {
                if let FranklinPriorityOp::Deposit(deposit) = &op.data {
                    // Address may be set to either sender or recipient.
                    deposit.from == address || deposit.to == address
                } else {
                    false
                }
            })
            .map(|(block, op)| {
                if block > max_block_number {
                    max_block_number = block;
                }

                OngoingDeposit::new(block, op)
            })
            .collect();

        let estimated_deposits_approval_block = if !deposits.is_empty() {
            // We have to wait `confirmations_for_eth_event` blocks after the most
            // recent deposit operation.
            Some(max_block_number + confirmations_for_eth_event)
        } else {
            // No ongoing deposits => no estimated block.
            None
        };

        Ok(OngoingDepositsResp {
            address,
            deposits,
            confirmations_for_eth_event,
            estimated_deposits_approval_block,
        })
    }
}

impl Rpc for RpcApp {
    fn account_info(
        &self,
        address: Address,
    ) -> Box<dyn futures01::Future<Item = AccountInfoResp, Error = Error> + Send> {
        let (account, tokens) = if let Ok((account, tokens)) = (|| -> Result<_> {
            let storage = self.access_storage()?;
            let account = storage
                .chain()
                .account_schema()
                .account_state_by_address(&address)
                .map_err(|_| Error::internal_error())?;
            let tokens = storage
                .tokens_schema()
                .load_tokens()
                .map_err(|_| Error::internal_error())?;
            Ok((account, tokens))
        })() {
            (account, tokens)
        } else {
            return Box::new(futures01::done(Err(Error::internal_error())));
        };

        let mut state_keeper_request_sender = self.state_keeper_request_sender.clone();
        let self_ = self.clone();
        let account_state_resp = async move {
            let state_keeper_response = oneshot::channel();
            state_keeper_request_sender
                .send(StateKeeperRequest::GetAccount(
                    address,
                    state_keeper_response.0,
                ))
                .await
                .expect("state keeper receiver dropped");
            let committed_account_state = state_keeper_response
                .1
                .await
                .map_err(|_| Error::internal_error())?;

            let (id, committed) = if let Some((id, account)) = committed_account_state {
                (
                    Some(id),
                    ResponseAccountState::try_to_restore(account, &tokens)?,
                )
            } else {
                (None, ResponseAccountState::default())
            };

            let verified = if let Some((_, account)) = account.verified {
                ResponseAccountState::try_to_restore(account, &tokens)?
            } else {
                ResponseAccountState::default()
            };

            let depositing_ops = self_.get_ongoing_deposits_impl(address).await?;
            let depositing = DepositingAccountBalances::from_pending_ops(depositing_ops, &tokens)?;

            Ok(AccountInfoResp {
                address,
                id,
                committed,
                verified,
                depositing,
            })
        };

        Box::new(account_state_resp.boxed().compat())
    }

    fn get_ongoing_deposits(
        &self,
        address: Address,
    ) -> Box<dyn futures01::Future<Item = OngoingDepositsResp, Error = Error> + Send> {
        let self_ = self.clone();
        let ongoing_deposits_resp = async move { self_.get_ongoing_deposits_impl(address).await };

        Box::new(ongoing_deposits_resp.boxed().compat())
    }

    fn ethop_info(&self, serial_id: u32) -> Result<ETHOpInfoResp> {
        let storage = self.access_storage()?;
        let executed_op = storage
            .chain()
            .operations_schema()
            .get_executed_priority_operation(serial_id)
            .map_err(|_| Error::internal_error())?;
        Ok(if let Some(executed_op) = executed_op {
            let block = storage
                .chain()
                .block_schema()
                .find_block_by_height_or_hash(executed_op.block_number.to_string());
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

    fn tx_info(&self, tx_hash: TxHash) -> Result<TransactionInfoResp> {
        let storage = self.access_storage()?;
        let stored_receipt = storage
            .chain()
            .operations_ext_schema()
            .tx_receipt(tx_hash.as_ref())
            .map_err(|_| Error::internal_error())?;
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

    fn tx_submit(
        &self,
        tx: Box<FranklinTx>,
        signature: Box<Option<TxEthSignature>>,
    ) -> Box<dyn futures01::Future<Item = TxHash, Error = Error> + Send> {
        if tx.is_close() {
            return Box::new(futures01::future::err(Error {
                code: RpcErrorCodes::AccountCloseDisabled.into(),
                message: "Account close tx is disabled.".to_string(),
                data: None,
            }));
        }

        let mut mempool_sender = self.mempool_request_sender.clone();
        let mempool_resp = async move {
            let hash = tx.hash();
            let mempool_resp = oneshot::channel();
            mempool_sender
                .send(MempoolRequest::NewTx(tx, signature, mempool_resp.0))
                .await
                .expect("mempool receiver dropped");
            let tx_add_result = mempool_resp.1.await.unwrap_or(Err(TxAddError::Other));

            tx_add_result.map(|_| hash).map_err(|e| Error {
                code: RpcErrorCodes::from(e).into(),
                message: e.to_string(),
                data: None,
            })
        };

        Box::new(mempool_resp.boxed().compat())
    }

    fn contract_address(&self) -> Result<ContractAddressResp> {
        let storage = self.access_storage()?;
        let config = storage
            .config_schema()
            .load_config()
            .map_err(|_| Error::internal_error())?;

        Ok(ContractAddressResp {
            main_contract: config.contract_addr.expect("server config"),
            gov_contract: config.gov_contract_addr.expect("server config"),
        })
    }

    fn tokens(&self) -> Result<HashMap<String, Token>> {
        let storage = self.access_storage()?;
        let mut tokens = storage
            .tokens_schema()
            .load_tokens()
            .map_err(|_| Error::internal_error())?;
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

    fn get_tx_fee(
        &self,
        _tx_type: TxFeeTypes,
        amount: BigDecimal,
        _token_like: TokenLike,
    ) -> Result<BigDecimal> {
        // first approximation - just give 1 percent
        Ok(closest_packable_fee_amount(&floor_big_decimal(
            &(amount / BigDecimal::from(100)),
        )))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn start_rpc_server(
    config_options: &ConfigurationOptions,
    connection_pool: ConnectionPool,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    eth_watcher_request_sender: mpsc::Sender<EthWatchRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    let addr = config_options.json_rpc_http_server_address;
    let confirmations_for_eth_event = config_options.confirmations_for_eth_event;
    std::thread::Builder::new()
        .name("json_rpc_http".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let mut io = IoHandler::new();

            let rpc_app = RpcApp {
                connection_pool,
                mempool_request_sender,
                state_keeper_request_sender,
                eth_watcher_request_sender,

                confirmations_for_eth_event,
            };
            rpc_app.extend(&mut io);

            let server = ServerBuilder::new(io).threads(1).start_http(&addr).unwrap();

            server.wait();
        })
        .expect("JSON-RPC http thread");
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn tx_fee_type_serialization() {
        #[derive(Debug, Serialize, Deserialize, PartialEq)]
        struct Query {
            tx_type: TxFeeTypes,
        }

        let cases = vec![
            (
                Query {
                    tx_type: TxFeeTypes::Withdraw,
                },
                r#"{"tx_type":"Withdraw"}"#,
            ),
            (
                Query {
                    tx_type: TxFeeTypes::Transfer,
                },
                r#"{"tx_type":"Transfer"}"#,
            ),
        ];
        for (query, json_str) in cases {
            let ser = serde_json::to_string(&query).expect("ser");
            assert_eq!(ser, json_str);
            let de = serde_json::from_str::<Query>(&ser).expect("de");
            assert_eq!(query, de);
        }
    }
}
