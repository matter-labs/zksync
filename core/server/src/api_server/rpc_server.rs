// Built-in deps
use std::boxed::Box;
use std::collections::HashMap;
use std::net::SocketAddr;

// External uses
use bigdecimal::BigDecimal;
use futures::{
    channel::{mpsc, oneshot},
    FutureExt, SinkExt, TryFutureExt,
};
use jsonrpc_core::{Error, ErrorCode, IoHandler, MetaIoHandler, Metadata, Middleware, Result};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use storage::chain::block::records::BlockDetails;
use storage::chain::operations::records::{
    StoredExecutedPriorityOperation
};
use storage::chain::operations_ext::records::TxReceiptResponse;
use web3::types::Address;
// Workspace uses
use models::{
    config_options::ThreadPanicNotify,
    node::{
        closest_packable_fee_amount,
        tx::{TxEthSignature, TxHash},
        Account, AccountId, FranklinTx, Nonce, PubKeyHash, Token, TokenId, TokenLike,
    },
    primitives::floor_big_decimal,
};
use storage::{ConnectionPool, StorageProcessor};
// Local uses
use crate::{
    mempool::{MempoolRequest, TxAddError},
    signature_checker::{VerifiedTx, VerifyTxSignatureRequest},
    state_keeper::StateKeeperRequest,
    utils::shared_lru_cache::SharedLruCache,
    utils::token_db_cache::TokenDBCache,
};

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

#[derive(Debug)]
pub enum RpcErrorCodes {
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
    cache_of_executed_priority_operation: SharedLruCache<u32, StoredExecutedPriorityOperation>,
    cache_of_block_info: SharedLruCache<i64, BlockDetails>,
    cache_of_transaction_receipts: SharedLruCache<Vec<u8>, TxReceiptResponse>,
    pub mempool_request_sender: mpsc::Sender<MempoolRequest>,
    pub state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
    pub connection_pool: ConnectionPool,
    pub sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    pub token_cache: TokenDBCache,
}

impl RpcApp {
    pub fn new(
        connection_pool: ConnectionPool,
        mempool_request_sender: mpsc::Sender<MempoolRequest>,
        state_keeper_request_sender: mpsc::Sender<StateKeeperRequest>,
        sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    ) -> Self {
        let token_cache = TokenDBCache::new(connection_pool.clone());

        RpcApp {
            cache_of_executed_priority_operation: SharedLruCache::new(2),
            cache_of_block_info: SharedLruCache::new(2),
            cache_of_transaction_receipts: SharedLruCache::new(2),
            connection_pool,
            mempool_request_sender,
            state_keeper_request_sender,
            sign_verify_request_sender,
            token_cache,
        }
    }

    pub fn extend<T: Metadata, S: Middleware<T>>(self, io: &mut MetaIoHandler<T, S>) {
        io.extend_with(self.to_delegate())
    }

    fn token_symbol_from_id(&self, token_id: TokenId) -> Result<String> {
        fn rpc_message(error: impl ToString) -> Error {
            Error {
                code: RpcErrorCodes::Other.into(),
                message: error.to_string(),
                data: None,
            }
        }

        let symbol = self
            .token_cache
            .get_token(token_id)
            .map_err(rpc_message)?
            .map(|t| t.symbol);

        match symbol {
            Some(symbol) => Ok(symbol),
            None => Err(rpc_message("Token not found in the DB")),
        }
    }

    /// Returns a message that user has to sign to send the transaction.
    /// If the transaction doesn't need a message signature, returns `None`.
    /// If any error is encountered during the message generation, returns `jsonrpc_core::Error`.
    fn get_tx_info_message_to_sign(&self, tx: &FranklinTx) -> Result<Option<String>> {
        match tx {
            FranklinTx::Transfer(tx) => Ok(Some(
                tx.get_ethereum_sign_message(&self.token_symbol_from_id(tx.token)?),
            )),
            FranklinTx::Withdraw(tx) => Ok(Some(
                tx.get_ethereum_sign_message(&self.token_symbol_from_id(tx.token)?),
            )),
            _ => Ok(None),
        }
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
        let executed_op =
            if let Some(executed_op) = self.cache_of_executed_priority_operation.get(&serial_id) {
                Some(executed_op)
            } else {
                let storage = self.access_storage()?;
                let executed_op = storage
                    .chain()
                    .operations_schema()
                    .get_executed_priority_operation(serial_id)
                    .map_err(|_| Error::internal_error())?;

                if let Some(executed_op) = executed_op.clone() {
                    self.cache_of_executed_priority_operation
                        .insert(serial_id, executed_op);
                }

                executed_op
            };
        Ok(if let Some(executed_op) = executed_op {
            let block = if let Some(block) = self.cache_of_block_info.get(&executed_op.block_number)
            {
                Some(block)
            } else {
                let storage = self.access_storage()?;
                let block = storage
                    .chain()
                    .block_schema()
                    .find_block_by_height_or_hash(executed_op.block_number.to_string());

                if let Some(block) = block.clone() {
                    if block.verified_at.is_some() {
                        self.cache_of_block_info
                            .insert(executed_op.block_number, block);
                    }
                }

                block
            };
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
        let stored_receipt = if let Some(tx_receipt) = self
            .cache_of_transaction_receipts
            .get(&tx_hash.as_ref().to_vec())
        {
            Some(tx_receipt)
        } else {
            let storage = self.access_storage()?;
            let tx_receipt = storage
                .chain()
                .operations_ext_schema()
                .tx_receipt(tx_hash.as_ref())
                .map_err(|_| Error::internal_error())?;

            if let Some(tx_receipt) = tx_receipt.clone() {
                if tx_receipt.verified {
                    self.cache_of_transaction_receipts
                        .insert(tx_hash.as_ref().to_vec(), tx_receipt);
                }
            }

            tx_receipt
        };
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

        let msg_to_sign = match self.get_tx_info_message_to_sign(&tx) {
            Ok(res) => res,
            Err(e) => return Box::new(futures01::future::err(e)),
        };

        let mut mempool_sender = self.mempool_request_sender.clone();
        let sign_verify_channel = self.sign_verify_request_sender.clone();
        let mempool_resp = async move {
            let verified_tx =
                verify_tx_info_message_signature(&tx, *signature, msg_to_sign, sign_verify_channel)
                    .await?;

            let hash = tx.hash();
            let mempool_resp = oneshot::channel();
            mempool_sender
                .send(MempoolRequest::NewTx(Box::new(verified_tx), mempool_resp.0))
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
    sign_verify_request_sender: mpsc::Sender<VerifyTxSignatureRequest>,
    panic_notify: mpsc::Sender<bool>,
) {
    std::thread::Builder::new()
        .name("json_rpc_http".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let mut io = IoHandler::new();

            let rpc_app = RpcApp::new(
                connection_pool,
                mempool_request_sender,
                state_keeper_request_sender,
                sign_verify_request_sender,
            );
            rpc_app.extend(&mut io);

            let server = ServerBuilder::new(io).threads(8).start_http(&addr).unwrap();

            server.wait();
        })
        .expect("JSON-RPC http thread");
}

async fn verify_tx_info_message_signature(
    tx: &FranklinTx,
    signature: Option<TxEthSignature>,
    msg_to_sign: Option<String>,
    mut req_channel: mpsc::Sender<VerifyTxSignatureRequest>,
) -> Result<VerifiedTx> {
    fn rpc_message(error: TxAddError) -> Error {
        Error {
            code: RpcErrorCodes::from(error).into(),
            message: error.to_string(),
            data: None,
        }
    }

    let eth_sign_data = match msg_to_sign {
        Some(message_to_sign) => {
            let signature =
                signature.ok_or_else(|| rpc_message(TxAddError::MissingEthSignature))?;

            Some((signature, message_to_sign))
        }
        None => None,
    };

    let resp = oneshot::channel();

    let request = VerifyTxSignatureRequest {
        tx: tx.clone(),
        eth_sign_data,
        response: resp.0,
    };

    // Send the check request.
    req_channel
        .send(request)
        .await
        .expect("verifier pool receiver dropped");

    // Wait for the check result.
    resp.1
        .await
        .map_err(|_| Error::internal_error())?
        .map_err(rpc_message)
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
