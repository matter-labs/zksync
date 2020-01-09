use bigdecimal::BigDecimal;
use jsonrpc_core::{Error, Result};
use jsonrpc_core::{IoHandler, MetaIoHandler, Metadata, Middleware};
use jsonrpc_derive::rpc;
use jsonrpc_http_server::ServerBuilder;
use models::config_options::ThreadPanicNotify;
use models::node::tx::TxHash;
use models::node::{Account, AccountAddress, AccountId, FranklinTx, Nonce, TokenId};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::mpsc;
use storage::{ConnectionPool, StorageProcessor, Token, TxAddError};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResponseAccountState {
    balances: HashMap<String, BigDecimal>,
    nonce: Nonce,
}

impl ResponseAccountState {
    pub fn try_to_restore(account: Account, tokens: &HashMap<TokenId, Token>) -> Result<Self> {
        let mut balances = HashMap::new();
        for (token_id, balance) in account.get_nonzero_balances() {
            if token_id == 0 {
                balances.insert("ETH".to_string(), balance);
            } else {
                let token = tokens.get(&token_id).ok_or_else(Error::internal_error)?;
                balances.insert(token.address.clone(), balance);
            }
        }

        Ok(Self {
            balances,
            nonce: account.nonce,
        })
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfoResp {
    address: AccountAddress,
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

#[rpc]
pub trait Rpc {
    #[rpc(name = "account_info")]
    fn account_info(&self, addr: AccountAddress) -> Result<AccountInfoResp>;
    #[rpc(name = "ethop_info")]
    fn ethop_info(&self, serial_id: u32) -> Result<ETHOpInfoResp>;
    #[rpc(name = "tx_info")]
    fn tx_info(&self, hash: TxHash) -> Result<TransactionInfoResp>;
    #[rpc(name = "tx_submit")]
    fn tx_submit(&self, tx: FranklinTx) -> Result<TxHash>;
    #[rpc(name = "contract_address")]
    fn contract_address(&self) -> Result<ContractAddressResp>;
    /// "ETH" | #ERC20_ADDRESS => {Token}
    #[rpc(name = "tokens")]
    fn tokens(&self) -> Result<HashMap<String, Token>>;
}

pub struct RpcApp {
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
            .access_storage()
            .map_err(|_| Error::internal_error())
    }
}

impl Rpc for RpcApp {
    fn account_info(&self, address: AccountAddress) -> Result<AccountInfoResp> {
        let storage = self.access_storage()?;
        let account = storage
            .account_state_by_address(&address)
            .map_err(|_| Error::internal_error())?;
        let tokens = storage.load_tokens().map_err(|_| Error::internal_error())?;

        let id = account.committed.as_ref().map(|(id, _)| *id);

        let commited = if let Some((_, account)) = account.committed {
            ResponseAccountState::try_to_restore(account, &tokens)?
        } else {
            ResponseAccountState::default()
        };

        let verified = if let Some((_, account)) = account.verified {
            ResponseAccountState::try_to_restore(account, &tokens)?
        } else {
            ResponseAccountState::default()
        };

        Ok(AccountInfoResp {
            address,
            id,
            committed: commited,
            verified,
        })
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

    fn tx_submit(&self, tx: FranklinTx) -> Result<TxHash> {
        let storage = self.access_storage()?;

        let tx_add_result = storage
            .mempool_add_tx(&tx)
            .map_err(|_| Error::internal_error())?;

        tx_add_result.map(|_| tx.hash()).map_err(|e| {
            let code = match &e {
                TxAddError::NonceTooLow => 101,
                TxAddError::InvalidSignature => 102,
                TxAddError::IncorrectTx => 103,
            };
            Error {
                code: code.into(),
                message: e.to_string(),
                data: None,
            }
        })
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
                    (token.address.clone(), token)
                }
            })
            .collect())
    }
}

pub fn start_rpc_server(
    addr: SocketAddr,
    connection_pool: ConnectionPool,
    panic_notify: mpsc::Sender<bool>,
) {
    std::thread::Builder::new()
        .name("json_rpc_http".to_string())
        .spawn(move || {
            let _panic_sentinel = ThreadPanicNotify(panic_notify);
            let mut io = IoHandler::new();

            let rpc_app = RpcApp { connection_pool };
            rpc_app.extend(&mut io);

            let server = ServerBuilder::new(io).threads(1).start_http(&addr).unwrap();

            server.wait();
        })
        .expect("JSON-RPC http thread");
}
