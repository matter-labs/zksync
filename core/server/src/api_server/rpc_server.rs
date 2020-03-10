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
use models::misc::constants::ETH_SIGNATURE_HEX_LENGTH;
use models::misc::utils::format_ether;
use models::node::tx::PackedEthSignature;
use models::node::tx::TxHash;
use models::node::{Account, AccountId, FranklinTx, Nonce, PubKeyHash, TokenId};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::string::ToString;
use storage::{ConnectionPool, StorageProcessor, Token};
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResp {
    address: Address,
    id: Option<AccountId>,
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

enum RpcErrorCodes {
    NonceMismatch = 101,
    IncorrectTx = 103,
    Other = 104,
    ChangePkNotAuthorized = 105,
    AccountCloseDisabled = 110,
    IncorrectEthSignature = 121,
}

impl From<TxAddError> for RpcErrorCodes {
    fn from(error: TxAddError) -> Self {
        match error {
            TxAddError::NonceMismatch => RpcErrorCodes::NonceMismatch,
            TxAddError::IncorrectTx => RpcErrorCodes::IncorrectTx,
            TxAddError::Other => RpcErrorCodes::Other,
            TxAddError::ChangePkNotAuthorized => RpcErrorCodes::ChangePkNotAuthorized,
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
        tx: FranklinTx,
        signature_string: Option<String>,
    ) -> Box<dyn futures01::Future<Item = TxHash, Error = Error> + Send>;
    #[rpc(name = "contract_address")]
    fn contract_address(&self) -> Result<ContractAddressResp>;
    /// "ETH" | #ERC20_ADDRESS => {Token}
    #[rpc(name = "tokens")]
    fn tokens(&self) -> Result<HashMap<String, Token>>;
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

    /// Returns token symbol string of TokenId.
    /// In case of failure, returns jsonrpc_core::Error,
    /// making it convenient to use in rpc methods.
    fn token_symbol_from_id(&self, token: TokenId) -> Result<String> {
        self.access_storage()?
            .token_symbol_from_id(token)
            .map_err(|_| Error::internal_error())?
            .ok_or(Error {
                code: RpcErrorCodes::IncorrectTx.into(),
                message: "No such token registered".into(),
                data: None,
            })
    }

    /// Returns message that user is to sign to send the transaction.
    /// If the transaction doesn't need a message signature, returns None.
    /// If any error is encountered during message generation, returns jsonrpc_core::Error
    fn get_tx_info_message_to_sign(&self, tx: &FranklinTx) -> Result<Option<String>> {
        match tx {
            FranklinTx::Transfer(tx) => {
                let token_symbol = self.token_symbol_from_id(tx.token)?;
                Ok(Some(format!(
                    "Transfer {} {}\nTo: {:?}\nNonce: {}\nFee: {} {}",
                    format_ether(&tx.amount),
                    &token_symbol,
                    tx.to,
                    tx.nonce,
                    format_ether(&tx.fee),
                    &token_symbol,
                )))
            }
            FranklinTx::Withdraw(tx) => {
                let token_symbol = self.token_symbol_from_id(tx.token)?;
                Ok(Some(format!(
                    "Withdraw {} {}\nTo: {:?}\nNonce: {}\nFee: {} {}",
                    format_ether(&tx.amount),
                    &token_symbol,
                    tx.to,
                    tx.nonce,
                    format_ether(&tx.fee),
                    &token_symbol,
                )))
            }
            _ => Ok(None),
        }
    }

    /// Checks that tx info message signature is valid.
    ///
    /// Needed for two-step verification, where user has to sign human-readable
    /// message with tx info with his eth signer in order to send transaction.
    ///
    /// If signature is correct, or tx doesn't need signature, returns Ok(())
    ///
    /// If any error encountered during signature verification,
    /// including incorrect signature, returns jsonrpc_core::Error
    fn verify_tx_info_message_signature(
        &self,
        tx: &FranklinTx,
        signature_string: Option<String>,
    ) -> Result<()> {
        fn rpc_message(message: impl ToString) -> Error {
            Error {
                code: RpcErrorCodes::IncorrectEthSignature.into(),
                message: message.to_string(),
                data: None,
            }
        }

        match self.get_tx_info_message_to_sign(&tx)? {
            Some(message_to_sign) => {
                let packed_signature = signature_string
                    .ok_or_else(|| rpc_message("Signature required"))
                    .and_then(|s| {
                        if s.len() != ETH_SIGNATURE_HEX_LENGTH {
                            return Err(rpc_message(format!(
                                "Signature must be {} character hex string",
                                ETH_SIGNATURE_HEX_LENGTH
                            )));
                        }
                        hex::decode(&s[2..]).map_err(rpc_message)
                    })
                    .and_then(|b| {
                        PackedEthSignature::deserialize_packed(&b).map_err(rpc_message)
                    })?;

                let signer_account = packed_signature
                    .signature_recover_signer(message_to_sign.as_bytes())
                    .map_err(rpc_message)?;

                if signer_account == tx.account() {
                    Ok(())
                } else {
                    Err(rpc_message("Signature incorrect"))
                }
            }
            None => Ok(()),
        }
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
                .account_state_by_address(&address)
                .map_err(|_| Error::internal_error())?;
            let tokens = storage.load_tokens().map_err(|_| Error::internal_error())?;
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
            .get_executed_priority_op(serial_id)
            .map_err(|_| Error::internal_error())?;
        Ok(if let Some(executed_op) = executed_op {
            let block = storage.handle_search(executed_op.block_number.to_string());
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
        tx: FranklinTx,
        signature_string: Option<String>,
    ) -> Box<dyn futures01::Future<Item = TxHash, Error = Error> + Send> {
        if tx.is_close() {
            return Box::new(futures01::future::err(Error {
                code: RpcErrorCodes::AccountCloseDisabled.into(),
                message: "Account close tx is disabled.".to_string(),
                data: None,
            }));
        }

        if let Err(error) = self.verify_tx_info_message_signature(&tx, signature_string) {
            return Box::new(futures01::future::err(error));
        }

        let mut mempool_sender = self.mempool_request_sender.clone();
        let mempool_resp = async move {
            let hash = tx.hash();
            let mempool_resp = oneshot::channel();
            mempool_sender
                .send(MempoolRequest::NewTx(Box::new(tx), mempool_resp.0))
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
        let config = storage.load_config().map_err(|_| Error::internal_error())?;

        Ok(ContractAddressResp {
            main_contract: config.contract_addr.expect("server config"),
            gov_contract: config.gov_contract_addr.expect("server config"),
        })
    }

    fn tokens(&self) -> Result<HashMap<String, Token>> {
        let storage = self.access_storage()?;
        let mut tokens = storage.load_tokens().map_err(|_| Error::internal_error())?;
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
