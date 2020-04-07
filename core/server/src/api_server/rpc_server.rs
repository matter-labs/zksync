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
use models::config_options::ThreadPanicNotify;
use models::node::tx::TxEthSignature;
use models::node::tx::TxHash;
use models::node::{
    closest_packable_fee_amount, Account, AccountId, FranklinTx, Nonce, PubKeyHash, Token, TokenId,
    TokenLike,
};
use models::primitives::floor_big_decimal;
use std::collections::HashMap;
use std::net::SocketAddr;
use storage::{ConnectionPool, StorageProcessor};
use web3::types::Address;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ResponseAccountState {
    pub balances: HashMap<String, BigDecimal>,
    pub nonce: Nonce,
    pub pub_key_hash: PubKeyHash,
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResp {
    pub address: Address,
    pub id: Option<AccountId>,
    pub committed: ResponseAccountState,
    pub verified: ResponseAccountState,
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
}

pub struct RpcApp {
    pub mempool_request_sender: mpsc::Sender<MempoolRequest>,
    pub state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    pub connection_pool: ConnectionPool,
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

            Ok(AccountInfoResp {
                address,
                id,
                committed,
                verified,
            })
        };

        Box::new(account_state_resp.boxed().compat())
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

pub fn start_rpc_server(
    addr: SocketAddr,
    connection_pool: ConnectionPool,
    mempool_request_sender: mpsc::Sender<MempoolRequest>,
    state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    std::thread::Builder::new()
        .name("json_rpc_http".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let mut io = IoHandler::new();

            let rpc_app = RpcApp {
                connection_pool,
                mempool_request_sender,
                state_keeper_request_sender,
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
